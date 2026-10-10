// Hidden WKWebView for the permission-free local playback integration test.
#import <Cocoa/Cocoa.h>
#import <WebKit/WebKit.h>

@interface Probe : NSObject <WKScriptMessageHandler, WKNavigationDelegate>
@end
@implementation Probe
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message {
    (void)controller;
    NSData *data = [NSJSONSerialization dataWithJSONObject:message.body options:0 error:nil];
    fwrite(data.bytes, 1, data.length, stdout); fputc('\n', stdout); fflush(stdout);
    [NSApp terminate:nil];
}
- (void)webView:(WKWebView *)webView didFailProvisionalNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    (void)webView; (void)navigation;
    fprintf(stderr, "WKWebView navigation failed: %s\n", error.localizedDescription.UTF8String);
    exit(1);
}
- (void)webView:(WKWebView *)webView didFailNavigation:(WKNavigation *)navigation withError:(NSError *)error {
    [self webView:webView didFailProvisionalNavigation:navigation withError:error];
}
@end

int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 2) return 2;
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
        Probe *probe = [Probe new];
        WKWebViewConfiguration *configuration = [WKWebViewConfiguration new];
        configuration.websiteDataStore = [WKWebsiteDataStore nonPersistentDataStore];
        configuration.mediaTypesRequiringUserActionForPlayback = WKAudiovisualMediaTypeNone;
        [configuration.userContentController addScriptMessageHandler:probe name:@"probe"];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 640, 480)
            styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        WKWebView *view = [[WKWebView alloc] initWithFrame:window.contentView.bounds configuration:configuration];
        view.navigationDelegate = probe;
        [window.contentView addSubview:view];
        // Keep window-backed WebKit media active while making the probe invisible.
        window.alphaValue = 0;
        [window orderFrontRegardless];
        [view loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:@(argv[1])]]];
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 25 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
            fprintf(stderr, "WKWebView playback probe timed out\n"); exit(1);
        });
        [NSApp run];
    }
    return 0;
}
