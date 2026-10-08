// Exercise actual CoreMedia/CoreVideo extraction without capturing the user's screen.
#import "screen_capture.m"
#include <assert.h>

static CMSampleBufferRef video_sample(void) {
    uint8_t *pixels = calloc(2, 16);
    assert(pixels);
    memcpy(pixels, (uint8_t[]){1, 2, 3, 4, 5, 6, 7, 8}, 8);
    memcpy(pixels + 16, (uint8_t[]){9, 10, 11, 12, 13, 14, 15, 16}, 8);
    CVPixelBufferRef image = NULL;
    assert(CVPixelBufferCreateWithBytes(NULL, 2, 2, kCVPixelFormatType_32BGRA, pixels, 16,
        NULL, NULL, NULL, &image) == kCVReturnSuccess);
    CMVideoFormatDescriptionRef format = NULL;
    assert(CMVideoFormatDescriptionCreateForImageBuffer(NULL, image, &format) == noErr);
    CMSampleTimingInfo timing = {CMTimeMake(1, 30), CMTimeMake(12345, 1000000), kCMTimeInvalid};
    CMSampleBufferRef sample = NULL;
    assert(CMSampleBufferCreateReadyWithImageBuffer(NULL, image, format, &timing, &sample) == noErr);
    CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sample, true);
    CFMutableDictionaryRef attachment = (CFMutableDictionaryRef)CFArrayGetValueAtIndex(attachments, 0);
    int complete = SCFrameStatusComplete;
    CFNumberRef value = CFNumberCreate(NULL, kCFNumberIntType, &complete);
    CFDictionarySetValue(attachment, (__bridge CFStringRef)SCStreamFrameInfoStatus, value);
    CFRelease(value);
    CFRelease(format);
    CFRelease(image);
    // The test keeps its external pixel storage alive through the buffer's final release.
    return sample;
}

static CMSampleBufferRef audio_sample(BOOL planar) {
    AudioStreamBasicDescription asbd = {0};
    asbd.mSampleRate = 48000;
    asbd.mFormatID = kAudioFormatLinearPCM;
    asbd.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked |
        (planar ? kAudioFormatFlagIsNonInterleaved : 0);
    asbd.mBytesPerPacket = asbd.mBytesPerFrame = planar ? 4 : 8;
    asbd.mFramesPerPacket = 1;
    asbd.mChannelsPerFrame = 2;
    asbd.mBitsPerChannel = 32;
    CMAudioFormatDescriptionRef description = NULL;
    assert(CMAudioFormatDescriptionCreate(NULL, &asbd, 0, NULL, 0, NULL, NULL, &description) == noErr);
    CMSampleTimingInfo timing = {CMTimeMake(1, 48000), CMTimeMake(12345, 1000000), kCMTimeInvalid};
    CMSampleBufferRef sample = NULL;
    assert(CMSampleBufferCreate(NULL, NULL, false, NULL, NULL, description, 2, 1, &timing, 0, NULL, &sample) == noErr);
    float left[] = {0.25f, 0.75f}, right[] = {-0.5f, 1.0f};
    float interleaved[] = {0.25f, -0.5f, 0.75f, 1.0f};
    AudioBufferList *list = calloc(1, sizeof(AudioBufferList) + sizeof(AudioBuffer));
    assert(list);
    list->mNumberBuffers = planar ? 2 : 1;
    list->mBuffers[0] = (AudioBuffer){planar ? 1 : 2, planar ? sizeof(left) : sizeof(interleaved), planar ? left : interleaved};
    if (planar) list->mBuffers[1] = (AudioBuffer){1, sizeof(right), right};
    assert(CMSampleBufferSetDataBufferFromAudioBufferList(sample, NULL, NULL, 0, list) == noErr);
    assert(CMSampleBufferSetDataReady(sample) == noErr);
    free(list);
    CFRelease(description);
    return sample;
}

static void check_packet(void *context, uint32_t kind, const uint8_t *bytes, size_t length,
                         uint32_t a, uint32_t b, uint32_t c, uint64_t timestamp) {
    uint32_t *counts = context;
    assert(timestamp == 12345);
    if (kind == 1) {
        assert(a == 2 && b == 2 && c == 0 && length == 16);
        for (size_t i = 0; i < length; ++i) assert(bytes[i] == i + 1);
        ++counts[0];
    } else {
        assert(kind == 2 && a == 48000 && b == 2 && c == 2 && length == 16);
        assert(memcmp(bytes, (float[]){0.25f, -0.5f, 0.75f, 1.0f}, length) == 0);
        ++counts[1];
    }
}

int main(void) {
    @autoreleasepool {
        VPPCapture *capture = [VPPCapture new];
        capture.queue = dispatch_queue_create("dev.videop2p.capture.test", DISPATCH_QUEUE_SERIAL);
        capture.packets = [NSMutableArray new];
        capture.active = capture.running = YES;
        CMSampleBufferRef video = video_sample();
        void *pixels = CVPixelBufferGetBaseAddress(CMSampleBufferGetImageBuffer(video));
        CMSampleBufferRef planar = audio_sample(YES), interleaved = audio_sample(NO);
        dispatch_sync(capture.queue, ^{
            for (int i = 0; i < 50; ++i) {
                [capture consumeSampleBuffer:video ofType:SCStreamOutputTypeScreen];
                if (@available(macOS 13.0, *))
                    [capture consumeSampleBuffer:(i % 2 ? planar : interleaved) ofType:SCStreamOutputTypeAudio];
            }
        });
        uint32_t counts[] = {0, 0};
        vpp_sck_poll((__bridge void *)capture, check_packet, counts);
        assert(counts[0] == 1);
        if (@available(macOS 13.0, *)) assert(counts[1] == 32);
        assert(capture.packets.count == 0);
        // Stop consumes the retained native handle and prevents further events.
        vpp_sck_destroy((__bridge_retained void *)capture);
        dispatch_sync(capture.queue, ^{
            [capture consumeSampleBuffer:video ofType:SCStreamOutputTypeScreen];
        });
        assert(capture.packets.count == 0);
        CFRelease(video); free(pixels);
        CFRelease(planar); CFRelease(interleaved);
        puts("ScreenCaptureKit buffer extraction, timing, backpressure, and stop tests passed");
    }
    return 0;
}
