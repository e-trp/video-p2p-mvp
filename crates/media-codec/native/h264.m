#import <Foundation/Foundation.h>
#import <VideoToolbox/VideoToolbox.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

typedef void (*VPPH264Callback)(void *, const uint8_t *, size_t, bool);

@interface VPPH264Encoder : NSObject
@property VTCompressionSessionRef session;
@property(nonatomic, strong) NSLock *lock;
@property(nonatomic, strong) NSData *output;
@property OSStatus outputStatus;
@property BOOL keyframe;
@property uint32_t width, height;
@end
@implementation VPPH264Encoder
- (void)dealloc {
    if (_session) { VTCompressionSessionInvalidate(_session); CFRelease(_session); }
}
@end

static int fail(char *error, size_t size, NSString *message, OSStatus status) {
    if (size) snprintf(error, size, "%s (OSStatus %d)", message.UTF8String, (int)status);
    return 1;
}

static void compressed(void *refcon, void *frameRefcon, OSStatus status, VTEncodeInfoFlags flags, CMSampleBufferRef sample) {
    (void)frameRefcon;
    @autoreleasepool {
        VPPH264Encoder *encoder = (__bridge VPPH264Encoder *)refcon;
        [encoder.lock lock];
        encoder.outputStatus = status;
        if (status || (flags & kVTEncodeInfo_FrameDropped) || !sample || !CMSampleBufferDataIsReady(sample)) {
            [encoder.lock unlock]; return;
        }
        NSArray *attachments = (__bridge NSArray *)CMSampleBufferGetSampleAttachmentsArray(sample, false);
        encoder.keyframe = ![[attachments.firstObject objectForKey:(__bridge NSString *)kCMSampleAttachmentKey_NotSync] boolValue];
        CMFormatDescriptionRef format = CMSampleBufferGetFormatDescription(sample);
        int lengthSize = 0;
        size_t count = 0;
        const uint8_t *parameter = NULL;
        size_t parameterSize = 0;
        status = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, 0, &parameter, &parameterSize, &count, &lengthSize);
        NSMutableData *annexB = [NSMutableData data];
        const uint8_t startCode[] = {0, 0, 0, 1};
        if (!status && encoder.keyframe) {
            for (size_t index = 0; index < count; ++index) {
                status = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, index, &parameter, &parameterSize, NULL, NULL);
                if (status) break;
                [annexB appendBytes:startCode length:4];
                [annexB appendBytes:parameter length:parameterSize];
            }
        }
        CMBlockBufferRef block = CMSampleBufferGetDataBuffer(sample);
        size_t size = block ? CMBlockBufferGetDataLength(block) : 0;
        NSMutableData *avcc = [NSMutableData dataWithLength:size];
        if (!status && size) status = CMBlockBufferCopyDataBytes(block, 0, size, avcc.mutableBytes);
        if (!status && (lengthSize < 1 || lengthSize > 4 || !size)) status = kVTParameterErr;
        const uint8_t *bytes = avcc.bytes;
        size_t offset = 0;
        while (!status && offset < size) {
            if (size - offset < (size_t)lengthSize) { status = kVTParameterErr; break; }
            uint32_t length = 0;
            for (int i = 0; i < lengthSize; ++i) length = (length << 8) | bytes[offset++];
            if (!length || length > size - offset) { status = kVTParameterErr; break; }
            [annexB appendBytes:startCode length:4];
            [annexB appendBytes:bytes + offset length:length];
            offset += length;
        }
        encoder.outputStatus = status;
        if (!status) encoder.output = annexB;
        [encoder.lock unlock];
    }
}

void *vpp_h264_create(uint32_t width, uint32_t height, uint32_t fps, uint32_t bitrate, char *error, size_t size) {
    @autoreleasepool {
        VPPH264Encoder *encoder = [VPPH264Encoder new];
        encoder.lock = [NSLock new];
        encoder.width = width; encoder.height = height;
        NSDictionary *attributes = @{
            (__bridge NSString *)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA),
            (__bridge NSString *)kCVPixelBufferWidthKey: @(width),
            (__bridge NSString *)kCVPixelBufferHeightKey: @(height),
            (__bridge NSString *)kCVPixelBufferIOSurfacePropertiesKey: @{}
        };
        VTCompressionSessionRef session = NULL;
        OSStatus status = VTCompressionSessionCreate(NULL, width, height, kCMVideoCodecType_H264, NULL,
            (__bridge CFDictionaryRef)attributes, NULL, compressed, (__bridge void *)encoder, &session);
        if (status) { fail(error, size, @"Cannot create VideoToolbox H.264 encoder", status); return NULL; }
        encoder.session = session;
        NSDictionary *properties = @{
            (__bridge NSString *)kVTCompressionPropertyKey_RealTime: @YES,
            (__bridge NSString *)kVTCompressionPropertyKey_AllowFrameReordering: @NO,
            (__bridge NSString *)kVTCompressionPropertyKey_ProfileLevel: (__bridge NSString *)kVTProfileLevel_H264_Baseline_3_1,
            (__bridge NSString *)kVTCompressionPropertyKey_ExpectedFrameRate: @(fps),
            (__bridge NSString *)kVTCompressionPropertyKey_AverageBitRate: @(bitrate),
            (__bridge NSString *)kVTCompressionPropertyKey_MaxKeyFrameInterval: @(fps * 2),
            (__bridge NSString *)kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration: @2
        };
        status = VTSessionSetProperties(session, (__bridge CFDictionaryRef)properties);
        if (!status) status = VTCompressionSessionPrepareToEncodeFrames(session);
        if (status) { fail(error, size, @"Cannot configure realtime H.264 encoding", status); return NULL; }
        return (__bridge_retained void *)encoder;
    }
}

int vpp_h264_encode(void *handle, const uint8_t *bytes, uint32_t width, uint32_t height, uint64_t pts,
                     bool forceKeyframe, VPPH264Callback callback, void *context, char *error, size_t size) {
    @autoreleasepool {
        VPPH264Encoder *encoder = (__bridge VPPH264Encoder *)handle;
        CVPixelBufferPoolRef pool = VTCompressionSessionGetPixelBufferPool(encoder.session);
        CVPixelBufferRef image = NULL;
        OSStatus status = pool ? CVPixelBufferPoolCreatePixelBuffer(NULL, pool, &image) : kVTAllocationFailedErr;
        if (status) return fail(error, size, @"Cannot allocate encoder pixel buffer", status);
        status = CVPixelBufferLockBaseAddress(image, 0);
        if (status) { CFRelease(image); return fail(error, size, @"Cannot lock encoder pixel buffer", status); }
        uint8_t *base = CVPixelBufferGetBaseAddress(image);
        size_t stride = CVPixelBufferGetBytesPerRow(image);
        if (!base || stride < (size_t)encoder.width * 4 ||
            CVPixelBufferGetPixelFormatType(image) != kCVPixelFormatType_32BGRA ||
            CVPixelBufferGetWidth(image) != encoder.width || CVPixelBufferGetHeight(image) != encoder.height) {
            CVPixelBufferUnlockBaseAddress(image, 0); CFRelease(image);
            return fail(error, size, @"Encoder pixel buffer does not match BGRA configuration", kVTParameterErr);
        }
        for (uint32_t row = 0; row < encoder.height; ++row) {
            uint32_t sourceRow = MIN(row, height - 1);
            memcpy(base + row * stride, bytes + (size_t)sourceRow * width * 4, (size_t)width * 4);
            if (encoder.width > width) memcpy(base + row * stride + width * 4, base + row * stride + (width - 1) * 4, 4);
        }
        CVPixelBufferUnlockBaseAddress(image, 0);
        [encoder.lock lock]; encoder.output = nil; encoder.outputStatus = noErr; [encoder.lock unlock];
        NSDictionary *properties = forceKeyframe ? @{(__bridge NSString *)kVTEncodeFrameOptionKey_ForceKeyFrame: @YES} : nil;
        status = VTCompressionSessionEncodeFrame(encoder.session, image, CMTimeMake((int64_t)pts, 1000000),
            kCMTimeInvalid, (__bridge CFDictionaryRef)properties, NULL, NULL);
        CFRelease(image);
        if (!status) status = VTCompressionSessionCompleteFrames(encoder.session, kCMTimeInvalid);
        [encoder.lock lock];
        if (!status) status = encoder.outputStatus;
        NSData *output = encoder.output;
        BOOL keyframe = encoder.keyframe;
        [encoder.lock unlock];
        if (status) return fail(error, size, @"H.264 frame encoding failed", status);
        // No Rust callback or input pointer is retained across this call.
        if (output.length) callback(context, output.bytes, output.length, keyframe);
        return 0;
    }
}

void vpp_h264_destroy(void *handle) {
    @autoreleasepool {
        VPPH264Encoder *encoder = (__bridge_transfer VPPH264Encoder *)handle;
        VTCompressionSessionCompleteFrames(encoder.session, kCMTimeInvalid);
        VTCompressionSessionInvalidate(encoder.session);
    }
}
