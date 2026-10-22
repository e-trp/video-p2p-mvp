// Opt-in visible source for ScreenCaptureKit QA. Plays only the supplied local WAV file.
#import <Cocoa/Cocoa.h>
#include <unistd.h>

@interface CaptureFixtureView : NSView
@end
@implementation CaptureFixtureView
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    [[NSColor colorWithSRGBRed:0.75 green:0.25 blue:0.125 alpha:1] setFill];
    NSRectFill(self.bounds);
    double phase = fmod(NSProcessInfo.processInfo.systemUptime, 3.0) / 3.0;
    [[NSColor whiteColor] setFill];
    NSRectFill(NSMakeRect(phase * (self.bounds.size.width - 40), 60, 40, 40));
    [@"VideoP2P native capture QA" drawAtPoint:NSMakePoint(20, 150)
        withAttributes:@{NSForegroundColorAttributeName: NSColor.whiteColor,
                         NSFontAttributeName: [NSFont systemFontOfSize:20]}];
}
@end

int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 2) {
            fprintf(stderr, "usage: native-capture-fixture WAV_PATH|--silent\n");
            return 2;
        }
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        NSSound *sound = nil;
        if (strcmp(argv[1], "--silent") != 0) {
            sound = [[NSSound alloc] initWithContentsOfFile:@(argv[1]) byReference:NO];
            if (!sound) { fprintf(stderr, "Cannot load fixture WAV file\n"); return 1; }
            sound.loops = YES;
            if (![sound play]) { fprintf(stderr, "Cannot play fixture WAV file\n"); return 1; }
        }
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(120, 120, 480, 270)
            styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
        window.title = @"VideoP2P Capture QA";
        CaptureFixtureView *view = [[CaptureFixtureView alloc] initWithFrame:window.contentView.bounds];
        window.contentView = view;
        [window makeKeyAndOrderFront:nil];
        [NSApp activate];
        NSTimer *animation = [NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *timer) {
            (void)timer;
            view.needsDisplay = YES;
        }];
        printf("window_id=%ld process_id=%d audio=%s\n", (long)window.windowNumber, getpid(), sound ? "wav" : "silent");
        fflush(stdout);
        // Bound accidental unattended runs; the QA runner normally terminates its own fixture sooner.
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 120 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
            [NSApp terminate:nil];
        });
        [NSApp run];
        [animation invalidate];
        [sound stop];
    }
    return 0;
}
