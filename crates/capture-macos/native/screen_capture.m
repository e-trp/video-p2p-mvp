#import <Foundation/Foundation.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import "screen_capture.h"
#include <math.h>

// All stream state belongs to one serial queue. Rust only receives owned copies.
@interface VPPPacket : NSObject
@property uint32_t type, a, b, c;
@property uint64_t timestamp;
@property(nonatomic, strong) NSData *data;
@end
@implementation VPPPacket
@end

@interface VPPContentResult : NSObject
@property(nonatomic, strong) NSCondition *condition;
@property(nonatomic, strong) SCShareableContent *content;
@property(nonatomic, strong) NSError *error;
@property BOOL done;
@end
@implementation VPPContentResult
@end

static void write_error(char *buffer, size_t size, NSString *message) {
    if (size) snprintf(buffer, size, "%s", message.UTF8String);
}

int vpp_sck_catalog(VPPSourceCallback callback, void *context, char *error, size_t size) {
    @autoreleasepool {
        // Passive refresh must never open a permission dialog.
        if (!CGPreflightScreenCaptureAccess()) {
            write_error(error, size, @"Screen Recording permission is required");
            return 1;
        }
        VPPContentResult *result = [VPPContentResult new];
        result.condition = [NSCondition new];
        [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:YES
            completionHandler:^(SCShareableContent *content, NSError *failure) {
                [result.condition lock];
                result.content = content;
                result.error = failure;
                result.done = YES;
                [result.condition signal];
                [result.condition unlock];
            }];
        [result.condition lock];
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
        while (!result.done && [result.condition waitUntilDate:deadline]) {}
        SCShareableContent *content = result.content;
        NSError *failure = result.error;
        BOOL done = result.done;
        [result.condition unlock];
        if (!done || !content || failure) {
            write_error(error, size, failure.localizedDescription ?: @"ScreenCaptureKit catalog timed out");
            return 1;
        }
        BOOL audio = NO;
        if (@available(macOS 13.0, *)) audio = YES;
        for (SCWindow *window in content.windows) {
            if (window.windowLayer != 0 || window.frame.size.width < 1 || window.frame.size.height < 1)
                continue;
            NSString *app = window.owningApplication.applicationName ?: @"";
            NSString *label = window.title.length ? window.title : (app.length ? app : @"Untitled window");
            NSString *identifier = [NSString stringWithFormat:@"macos-window-%u", window.windowID];
            callback(context, 0, identifier.UTF8String, label.UTF8String, app.UTF8String, audio);
        }
        for (SCDisplay *display in content.displays) {
            NSString *identifier = [NSString stringWithFormat:@"macos-display-%u", display.displayID];
            NSString *label = [NSString stringWithFormat:@"Display %u", display.displayID];
            callback(context, 2, identifier.UTF8String, label.UTF8String, "", audio);
            for (SCRunningApplication *app in content.applications) {
                BOOL visible = NO;
                for (SCWindow *window in content.windows) {
                    if (window.owningApplication.processID == app.processID && window.windowLayer == 0 &&
                        CGRectIntersectsRect(window.frame, display.frame)) { visible = YES; break; }
                }
                if (!visible) continue;
                identifier = [NSString stringWithFormat:@"macos-application-%d-%u", app.processID, display.displayID];
                label = [NSString stringWithFormat:@"All windows on display %u", display.displayID];
                callback(context, 1, identifier.UTF8String, label.UTF8String, app.applicationName.UTF8String, audio);
            }
        }
        return 0;
    }
}

@interface VPPCapture : NSObject <SCStreamOutput, SCStreamDelegate>
@property(nonatomic, strong) dispatch_queue_t queue;
@property(nonatomic, strong) SCStream *stream;
@property(nonatomic, strong) NSMutableArray<VPPPacket *> *packets;
@property BOOL active, running;
@end

@implementation VPPCapture
- (void)enqueue:(uint32_t)type data:(NSData *)data a:(uint32_t)a b:(uint32_t)b
               c:(uint32_t)c timestamp:(uint64_t)timestamp {
    // Keep the newest video frame and at most 32 audio buffers. Control events survive backpressure.
    NSUInteger count = 0;
    for (NSInteger i = (NSInteger)self.packets.count - 1; i >= 0; --i) {
        if (self.packets[i].type != type) continue;
        if (type == 1 || (type == 2 && ++count >= 32)) [self.packets removeObjectAtIndex:(NSUInteger)i];
    }
    VPPPacket *packet = [VPPPacket new];
    packet.type = type; packet.data = data; packet.a = a; packet.b = b; packet.c = c;
    packet.timestamp = timestamp;
    [self.packets addObject:packet];
}
- (void)fail:(NSString *)message {
    if (!self.active) return;
    self.active = NO;
    self.running = NO;
    [self.packets removeAllObjects];
    [self enqueue:3 data:[message dataUsingEncoding:NSUTF8StringEncoding] a:0 b:0 c:0 timestamp:0];
    [self.stream stopCaptureWithCompletionHandler:nil];
}
- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error {
    (void)stream;
    dispatch_async(self.queue, ^{ [self fail:error.localizedDescription]; });
}
- (void)stream:(SCStream *)stream didOutputSampleBuffer:(CMSampleBufferRef)sample
               ofType:(SCStreamOutputType)type {
    (void)stream;
    [self consumeSampleBuffer:sample ofType:type];
}
- (void)consumeSampleBuffer:(CMSampleBufferRef)sample ofType:(SCStreamOutputType)type {
    @autoreleasepool {
        if (!self.active || !CMSampleBufferIsValid(sample) || !CMSampleBufferDataIsReady(sample)) return;
        CMTime time = CMSampleBufferGetPresentationTimeStamp(sample);
        if (!CMTIME_IS_NUMERIC(time) || time.value < 0) return;
        CMTime micros = CMTimeConvertScale(time, 1000000, kCMTimeRoundingMethod_RoundTowardZero);
        if (!CMTIME_IS_NUMERIC(micros) || micros.value < 0) return;
        uint64_t timestamp = (uint64_t)micros.value;
        if (type == SCStreamOutputTypeScreen) {
            NSArray *attachments = (__bridge NSArray *)CMSampleBufferGetSampleAttachmentsArray(sample, false);
            NSNumber *status = [attachments.firstObject objectForKey:SCStreamFrameInfoStatus];
            if (!status || status.integerValue != SCFrameStatusComplete) return;
            CVPixelBufferRef image = CMSampleBufferGetImageBuffer(sample);
            if (!image || CVPixelBufferGetPixelFormatType(image) != kCVPixelFormatType_32BGRA) return;
            if (CVPixelBufferLockBaseAddress(image, kCVPixelBufferLock_ReadOnly) != kCVReturnSuccess) return;
            size_t width = CVPixelBufferGetWidth(image), height = CVPixelBufferGetHeight(image);
            size_t stride = CVPixelBufferGetBytesPerRow(image);
            const uint8_t *base = CVPixelBufferGetBaseAddress(image);
            if (base && width && height && width <= 4096 && height <= 4096 && stride >= width * 4) {
                NSMutableData *packed = [NSMutableData dataWithLength:width * height * 4];
                for (size_t row = 0; row < height; ++row)
                    memcpy((uint8_t *)packed.mutableBytes + row * width * 4, base + row * stride, width * 4);
                [self enqueue:1 data:packed a:(uint32_t)width b:(uint32_t)height c:0 timestamp:timestamp];
            }
            CVPixelBufferUnlockBaseAddress(image, kCVPixelBufferLock_ReadOnly);
        } else {
            if (@available(macOS 13.0, *)) {
                if (type != SCStreamOutputTypeAudio) return;
                CMAudioFormatDescriptionRef description = CMSampleBufferGetFormatDescription(sample);
                const AudioStreamBasicDescription *format = description ? CMAudioFormatDescriptionGetStreamBasicDescription(description) : NULL;
                if (!format || format->mFormatID != kAudioFormatLinearPCM || format->mBitsPerChannel != 32 ||
                    !(format->mFormatFlags & kAudioFormatFlagIsFloat) ||
                    (format->mFormatFlags & kAudioFormatFlagIsBigEndian) || format->mChannelsPerFrame < 1 ||
                    format->mChannelsPerFrame > 2 || format->mSampleRate != 48000) {
                    [self fail:@"ScreenCaptureKit returned an unsupported PCM audio format"];
                    return;
                }
                CMItemCount frames = CMSampleBufferGetNumSamples(sample);
                if (frames <= 0 || frames > 48000) return;
                size_t listSize = 0;
                OSStatus result = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sample, &listSize,
                    NULL, 0, NULL, NULL, kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, NULL);
                if (result != noErr || listSize < sizeof(AudioBufferList)) return;
                AudioBufferList *list = malloc(listSize);
                if (!list) return;
                CMBlockBufferRef block = NULL;
                result = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sample, NULL, list, listSize,
                    NULL, NULL, kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment, &block);
                uint32_t channels = format->mChannelsPerFrame;
                BOOL planar = (format->mFormatFlags & kAudioFormatFlagIsNonInterleaved) != 0;
                BOOL valid = result == noErr && list->mNumberBuffers == (planar ? channels : 1);
                if (valid) {
                    for (uint32_t i = 0; i < list->mNumberBuffers; ++i) {
                        const AudioBuffer *buffer = &list->mBuffers[i];
                        uint32_t expectedChannels = planar ? 1 : channels;
                        if (!buffer->mData || buffer->mNumberChannels != expectedChannels ||
                            buffer->mDataByteSize < (size_t)frames * expectedChannels * sizeof(float)) valid = NO;
                    }
                }
                if (valid) {
                    NSMutableData *pcm = [NSMutableData dataWithLength:(size_t)frames * channels * sizeof(float)];
                    float *destination = pcm.mutableBytes;
                    for (CMItemCount frame = 0; frame < frames; ++frame)
                        for (uint32_t channel = 0; channel < channels; ++channel) {
                            const float *source = list->mBuffers[planar ? channel : 0].mData;
                            destination[frame * channels + channel] = source[planar ? frame : frame * channels + channel];
                        }
                    [self enqueue:2 data:pcm a:48000 b:channels c:(uint32_t)frames timestamp:timestamp];
                }
                if (block) CFRelease(block);
                free(list);
            }
        }
    }
}
@end

void *vpp_sck_start(uint32_t kind, uint32_t identifier, uint32_t displayID,
                    bool audio, uint32_t fps, uint32_t width, uint32_t height) {
    @autoreleasepool {
        VPPCapture *capture = [VPPCapture new];
        capture.queue = dispatch_queue_create("dev.videop2p.capture", DISPATCH_QUEUE_SERIAL);
        capture.packets = [NSMutableArray new];
        capture.active = YES;
        [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:YES
            completionHandler:^(SCShareableContent *content, NSError *error) {
                dispatch_async(capture.queue, ^{
                    if (!capture.active) return;
                    if (!content || error) { [capture fail:error.localizedDescription ?: @"Shareable content unavailable"]; return; }
                    SCContentFilter *filter = nil;
                    CGSize sourceSize = CGSizeZero;
                    if (kind == 0) {
                        for (SCWindow *window in content.windows) {
                            if (window.windowID != identifier) continue;
                            filter = [[SCContentFilter alloc] initWithDesktopIndependentWindow:window];
                            sourceSize = window.frame.size;
                            break;
                        }
                    } else {
                        for (SCDisplay *display in content.displays) {
                            if (display.displayID != (kind == 2 ? identifier : displayID)) continue;
                            sourceSize = display.frame.size;
                            if (kind == 2) filter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:@[]];
                            else for (SCRunningApplication *app in content.applications) {
                                if ((uint32_t)app.processID == identifier)
                                    filter = [[SCContentFilter alloc] initWithDisplay:display includingApplications:@[app] exceptingWindows:@[]];
                            }
                            break;
                        }
                    }
                    if (!filter || sourceSize.width <= 0 || sourceSize.height <= 0) {
                        [capture fail:@"Selected ScreenCaptureKit source is no longer available"]; return;
                    }
                    SCStreamConfiguration *config = [SCStreamConfiguration new];
                    double scale = fmin((double)width / sourceSize.width, (double)height / sourceSize.height);
                    config.width = MAX(1, (size_t)floor(sourceSize.width * scale));
                    config.height = MAX(1, (size_t)floor(sourceSize.height * scale));
                    config.minimumFrameInterval = CMTimeMake(1, (int32_t)fps);
                    config.pixelFormat = kCVPixelFormatType_32BGRA;
                    config.queueDepth = 3;
                    config.showsCursor = YES;
                    if (@available(macOS 13.0, *)) {
                        config.capturesAudio = audio;
                        config.sampleRate = 48000;
                        config.channelCount = 2;
                        config.excludesCurrentProcessAudio = YES;
                    }
                    capture.stream = [[SCStream alloc] initWithFilter:filter configuration:config delegate:capture];
                    NSError *outputError = nil;
                    if (![capture.stream addStreamOutput:capture type:SCStreamOutputTypeScreen
                            sampleHandlerQueue:capture.queue error:&outputError]) {
                        [capture fail:outputError.localizedDescription ?: @"Cannot attach video output"]; return;
                    }
                    if (@available(macOS 13.0, *)) {
                        if (audio && ![capture.stream addStreamOutput:capture type:SCStreamOutputTypeAudio
                                sampleHandlerQueue:capture.queue error:&outputError]) {
                            [capture fail:outputError.localizedDescription ?: @"Cannot attach audio output"]; return;
                        }
                    }
                    [capture.stream startCaptureWithCompletionHandler:^(NSError *failure) {
                        dispatch_async(capture.queue, ^{
                            if (!capture.active) { [capture.stream stopCaptureWithCompletionHandler:nil]; return; }
                            if (failure) { [capture fail:failure.localizedDescription]; return; }
                            capture.running = YES;
                            // A Started event precedes any queued samples on the first poll.
                            VPPPacket *started = [VPPPacket new];
                            started.type = 0; started.data = [NSData data];
                            [capture.packets insertObject:started atIndex:0];
                        });
                    }];
                });
            }];
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 8 * NSEC_PER_SEC), capture.queue, ^{
            if (capture.active && !capture.running) [capture fail:@"ScreenCaptureKit startup timed out"];
        });
        return (__bridge_retained void *)capture;
    }
}

void vpp_sck_poll(void *handle, VPPEventCallback callback, void *context) {
    @autoreleasepool {
        VPPCapture *capture = (__bridge VPPCapture *)handle;
        __block NSArray<VPPPacket *> *packets;
        dispatch_sync(capture.queue, ^{
            if (!capture.running && capture.active) {
                packets = @[];
            } else {
                packets = [capture.packets copy];
                [capture.packets removeAllObjects];
            }
        });
        for (VPPPacket *packet in packets)
            callback(context, packet.type, packet.data.bytes, packet.data.length,
                packet.a, packet.b, packet.c, packet.timestamp);
    }
}

void vpp_sck_destroy(void *handle) {
    @autoreleasepool {
        VPPCapture *capture = (__bridge_transfer VPPCapture *)handle;
        dispatch_sync(capture.queue, ^{
            capture.active = NO;
            capture.running = NO;
            [capture.packets removeAllObjects];
            NSError *error = nil;
            [capture.stream removeStreamOutput:capture type:SCStreamOutputTypeScreen error:&error];
            if (@available(macOS 13.0, *))
                [capture.stream removeStreamOutput:capture type:SCStreamOutputTypeAudio error:&error];
            [capture.stream stopCaptureWithCompletionHandler:nil];
            capture.stream = nil;
        });
    }
}
