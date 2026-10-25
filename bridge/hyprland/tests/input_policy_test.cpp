#include "input_policy.hpp"
#include <cassert>
int main() {
    assert(writy::modifierSymbol(XKB_KEY_Control_L));
    assert(writy::modifierSymbol(XKB_KEY_Alt_R));
    assert(writy::modifierSymbol(XKB_KEY_ISO_Level3_Shift));
    assert(!writy::modifierSymbol(XKB_KEY_v));
    assert(writy::acceptanceSymbol(XKB_KEY_Return, (1U << 2) | (1U << 3)));
    assert(writy::acceptanceSymbol(XKB_KEY_KP_Enter, (1U << 2) | (1U << 3)));
    assert(!writy::acceptanceSymbol(XKB_KEY_Return, 1U << 2));
    assert(!writy::acceptanceSymbol(XKB_KEY_v, (1U << 2) | (1U << 3)));
    assert(writy::navigationSymbol(XKB_KEY_BackSpace));
    assert(!writy::navigationSymbol(XKB_KEY_d));
}
