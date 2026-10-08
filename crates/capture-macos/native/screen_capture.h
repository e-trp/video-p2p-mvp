#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Callbacks run synchronously during catalog/poll. No Rust pointer is retained.
typedef void (*VPPSourceCallback)(void *, uint32_t, const char *, const char *, const char *, bool);
typedef void (*VPPEventCallback)(void *, uint32_t, const uint8_t *, size_t,
                                 uint32_t, uint32_t, uint32_t, uint64_t);
int vpp_sck_catalog(VPPSourceCallback, void *, char *, size_t);
void *vpp_sck_start(uint32_t kind, uint32_t identifier, uint32_t display,
                    bool audio, uint32_t fps, uint32_t width, uint32_t height);
void vpp_sck_poll(void *, VPPEventCallback, void *);
void vpp_sck_destroy(void *);
