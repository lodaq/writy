#pragma once
#include <cstdint>
#include <xkbcommon/xkbcommon-keysyms.h>

namespace writy {
constexpr bool modifierSymbol(uint32_t symbol) {
    return (symbol >= XKB_KEY_Shift_L && symbol <= XKB_KEY_Hyper_R) ||
           symbol == XKB_KEY_ISO_Level3_Shift || symbol == XKB_KEY_ISO_Level5_Shift || symbol == XKB_KEY_Mode_switch;
}
// Compositor modifier bits; the binding dispatches acceptance, never this observer.
constexpr bool acceptanceSymbol(uint32_t symbol, uint32_t modifiers) {
    return (symbol == XKB_KEY_Return || symbol == XKB_KEY_KP_Enter) &&
           (modifiers & ((1U << 2) | (1U << 3))) == ((1U << 2) | (1U << 3)) &&
           (modifiers & ((1U << 0) | (1U << 6))) == 0;
}
constexpr bool navigationSymbol(uint32_t symbol) {
    return symbol == XKB_KEY_Return || symbol == XKB_KEY_KP_Enter || symbol == XKB_KEY_Tab || symbol == XKB_KEY_ISO_Left_Tab ||
           symbol == XKB_KEY_Escape || symbol == XKB_KEY_Left || symbol == XKB_KEY_Right || symbol == XKB_KEY_Up ||
           symbol == XKB_KEY_Down || symbol == XKB_KEY_Home || symbol == XKB_KEY_End || symbol == XKB_KEY_Page_Up ||
           symbol == XKB_KEY_Page_Down || symbol == XKB_KEY_Delete || symbol == XKB_KEY_BackSpace;
}
}
