#include <hyprland/src/plugins/PluginAPI.hpp>
#include <hyprland/src/Compositor.hpp>
#include <hyprland/src/output/Monitor.hpp>
#include <hyprland/src/managers/SeatManager.hpp>
#include <hyprland/src/desktop/view/LayerSurface.hpp>
#include <hyprland/src/desktop/state/FocusState.hpp>
#include <hyprland/src/desktop/view/Window.hpp>
#include <hyprland/src/managers/input/InputManager.hpp>
#include <hyprland/src/managers/eventLoop/EventLoopManager.hpp>
#include <hyprland/src/managers/SessionLockManager.hpp>
#include <nlohmann/json.hpp>
#include <linux/input-event-codes.h>
#include <chrono>
#include <array>
#include <cstdlib>
#include <memory>
#include <unordered_map>
#include "transport.hpp"
#include "input_policy.hpp"

using Json = nlohmann::json;
using Clock = std::chrono::steady_clock;
using namespace std::chrono_literals;

namespace {
struct Device {
    WP<IKeyboard> keyboard;
    std::string id;
    std::vector<CHyprSignalListener> listeners;
};

class Bridge {
  public:
    explicit Bridge(std::string path) : transport(std::move(path)) {
        listeners.push_back(Event::bus()->m_events.input.keyboard.focus.listen([this] { invalidate("focus"); }));
        listeners.push_back(Event::bus()->m_events.window.active.listen([this] { invalidate("window"); }));
        listeners.push_back(Event::bus()->m_events.input.keyboard.layout.listen([this] { invalidate("layout"); }));
        listeners.push_back(Event::bus()->m_events.input.mouse.button.listen([this](IPointer::SButtonEvent event, Event::SCallbackInfo&) {
            if (event.state != WL_POINTER_BUTTON_STATE_PRESSED) return;
            // HUD controls must survive their own press/release cycle. A click in
            // another surface still invalidates projected text before any edit.
            const auto resource = g_pSeatManager->m_state.pointerFocus.lock();
            const auto surface = resource ? Desktop::View::CWLSurface::fromResource(resource) : nullptr;
            const auto view = surface ? surface->view() : nullptr;
            const auto layer = view ? Desktop::View::CLayerSurface::fromView(view) : nullptr;
            if (layer && layer->m_namespace == "writy") return;
            invalidate("pointer");
        }));
        listeners.push_back(Event::bus()->m_events.config.reloaded.listen([this] { invalidate("config"); reconcileDevices(); }));
        listeners.push_back(g_pSessionLockManager->m_events.lock.listen([this] { invalidate("session-lock"); }));
        listeners.push_back(g_pSessionLockManager->m_events.unlock.listen([this] { invalidate("session-unlock"); }));
        reconcileDevices();
        timer = makeShared<CEventLoopTimer>(20ms, [this](SP<CEventLoopTimer> self, void*) {
            tick();
            self->updateTimeout(20ms);
        }, nullptr);
        g_pEventLoopManager->addTimer(timer);
    }
    ~Bridge() {
        if (timer) { timer->cancel(); g_pEventLoopManager->removeTimer(timer); timer.reset(); }
        listeners.clear();
        devices.clear();
    }

  private:
    writy::Transport transport;
    uint64_t seq = 0, focusEpoch = 0, nextDevice = 0;
    bool capture = false;
    Clock::time_point leaseExpiry{}, retryAfter{};
    SP<CEventLoopTimer> timer;
    std::vector<CHyprSignalListener> listeners;
    std::unordered_map<IKeyboard*, Device> devices;

    void emit(Json message) {
        message["seq"] = ++seq;
        message["focus_epoch"] = focusEpoch;
        auto line = message.dump(-1, ' ', false, Json::error_handler_t::replace);
        if (line.size() >= 1048576) { capture = false; transport.disconnect(); return; }
        if (!transport.enqueue(std::move(line))) capture = false;
        transport.flush();
    }
    std::array<int,4> lastBounds{};
    std::array<int,4> outputBounds() const {
        const auto window=Desktop::focusState()->window();
        const auto monitor=window ? window->m_monitor.lock() : nullptr;
        return monitor ? std::array<int,4>{static_cast<int>(monitor->m_position.x),static_cast<int>(monitor->m_position.y),static_cast<int>(monitor->m_size.x),static_cast<int>(monitor->m_size.y)} : std::array<int,4>{};
    }
    void focus() {
        lastBounds=outputBounds();
        const auto window = Desktop::focusState()->window();
        const auto monitor = window ? window->m_monitor.lock() : nullptr;
        emit({{"type", "focus"}, {"app_id", window ? window->m_class : ""},
              {"monitor", monitor ? monitor->m_name : ""},
              {"monitor_geometry", monitor ? Json(lastBounds) : Json(nullptr)},
              {"pid", window ? static_cast<uint32_t>(window->getPID()) : 0U},
              {"locked", g_pSessionLockManager->isSessionLocked()},
              {"window_id", window ? std::format("0x{:x}", reinterpret_cast<uintptr_t>(window.get())) : ""}});
    }
    void invalidate(const char* reason) {
        capture = false;
        ++focusEpoch;
        emit({{"type", "reset"}, {"reason", reason}});
        focus();
    }
    void keymap(const Device& device) {
        const auto keyboard = device.keyboard.lock();
        if (!keyboard) return;
        // The compositor already maintains a v1 serialization. Never serialize/compile it per key.
        emit({{"type", "keymap"}, {"device", device.id}, {"keymap", keyboard->m_xkbKeymapV1String},
              {"name", keyboard->m_hlName}, {"wayland_virtual", keyboard->isVirtual()},
              {"repeat_rate", keyboard->m_repeatRate}, {"repeat_delay", keyboard->m_repeatDelay}});
    }
    void modifiers(const Device& device) {
        const auto keyboard = device.keyboard.lock();
        if (!keyboard) return;
        const auto& state = keyboard->m_modifiersState;
        emit({{"type", "modifiers"}, {"device", device.id}, {"depressed", state.depressed},
              {"latched", state.latched}, {"locked", state.locked}, {"group", state.group}});
    }
    void onKey(IKeyboard* identity, IKeyboard::SKeyEvent event) {
        const auto found = devices.find(identity);
        if (found == devices.end()) return;
        const auto keyboard = found->second.keyboard.lock();
        if (!keyboard || !keyboard->m_enabled || !keyboard->m_allowed || g_pSessionLockManager->isSessionLocked()) return;
        const auto symbol = keyboard->m_xkbState ? xkb_state_key_get_one_sym(keyboard->m_xkbState, event.keycode + 8) : XKB_KEY_NoSymbol;
        if (event.state == WL_KEYBOARD_KEY_STATE_PRESSED && !writy::modifierSymbol(symbol)) {
            // Preserve the candidate while the compositor dispatches the dedicated acceptance key.
            const auto modifiers = keyboard->getModifiers() | g_pInputManager->getModsFromAllKBs();
            if (writy::acceptanceSymbol(symbol, modifiers)) return;
            const bool shortcut = (modifiers & (HL_MODIFIER_CTRL | HL_MODIFIER_ALT | HL_MODIFIER_META)) != 0;
            const bool navigation = writy::navigationSymbol(symbol);
            if (shortcut || navigation) { invalidate(shortcut ? "shortcut" : "navigation"); return; }
        }
        if (!capture || Clock::now() >= leaseExpiry) return;
        emit({{"type", "key"}, {"device", found->second.id}, {"keycode", event.keycode},
              {"state", static_cast<uint32_t>(event.state)}, {"time_ms", event.timeMs}});
    }
    void reconcileDevices() {
        if (std::erase_if(devices, [](const auto& entry) { return entry.second.keyboard.expired(); }) != 0)
            invalidate("device-removed");
        for (const auto& keyboard : g_pInputManager->m_keyboards) {
            if (devices.contains(keyboard.get())) continue;
            auto& device = devices[keyboard.get()];
            device.keyboard = keyboard;
            device.id = "keyboard-" + std::to_string(++nextDevice);
            auto* identity = keyboard.get();
            device.listeners.push_back(keyboard->m_keyboardEvents.key.listen([this, identity](IKeyboard::SKeyEvent event) { onKey(identity, event); }));
            device.listeners.push_back(keyboard->m_keyboardEvents.keymap.listen([this, identity] {
                invalidate("keymap"); keymap(devices.at(identity)); modifiers(devices.at(identity));
            }));
            device.listeners.push_back(keyboard->m_keyboardEvents.modifiers.listen([this, identity] { modifiers(devices.at(identity)); }));
            device.listeners.push_back(keyboard->m_keyboardEvents.repeatInfo.listen([this, identity] { keymap(devices.at(identity)); }));
            invalidate("device-added");
            keymap(device); modifiers(device);
        }
    }
    void tick() {
        try {
            reconcileDevices();
            if (!transport.connected() && Clock::now() >= retryAfter) {
                retryAfter = Clock::now() + 1s;
                if (transport.poll()) {
                    capture = false;
                    emit({{"type", "hello"}, {"version", 1}, {"hyprland_abi", __hyprland_api_get_hash()}});
                    invalidate("connected");
                    for (const auto& [_, device] : devices) { keymap(device); modifiers(device); }
                }
            }
            if (transport.connected() && outputBounds()!=lastBounds) focus();
            for (const auto& line : transport.readLines()) {
                const auto message = Json::parse(line, nullptr, false);
                if (!message.is_object() || message.value("type", "") != "capture" || !message.contains("enabled") ||
                    !message["enabled"].is_boolean() || !message.contains("focus_epoch") || !message["focus_epoch"].is_number_unsigned()) continue;
                if (message["focus_epoch"].get<uint64_t>() != focusEpoch) continue;
                capture = message["enabled"].get<bool>() && !g_pSessionLockManager->isSessionLocked();
                leaseExpiry = Clock::now() + 1500ms;
            }
            if (capture && Clock::now() >= leaseExpiry) invalidate("lease-expired");
            if (!transport.connected()) capture = false;
            transport.flush();
        } catch (...) {
            capture = false;
            transport.disconnect();
        }
    }
};
std::unique_ptr<Bridge> bridge;
}

APICALL EXPORT std::string PLUGIN_API_VERSION() { return HYPRLAND_API_VERSION; }
APICALL EXPORT PLUGIN_DESCRIPTION_INFO PLUGIN_INIT(HANDLE) {
    if (std::string_view(__hyprland_api_get_hash()) != __hyprland_api_get_client_hash())
        throw std::runtime_error("Writy bridge: compositor/header ABI mismatch; rebuild against installed Hyprland");
    const char* overridePath = std::getenv("WRITY_BRIDGE_SOCKET");
    const char* runtime = std::getenv("XDG_RUNTIME_DIR");
    if (!overridePath && !runtime) throw std::runtime_error("Writy bridge requires XDG_RUNTIME_DIR");
    bridge = std::make_unique<Bridge>(overridePath ? overridePath : std::string(runtime) + "/writy/bridge.sock");
    return {"writy", "Writy nonblocking input observation bridge", "Writy contributors", "0.1.0"};
}
APICALL EXPORT void PLUGIN_EXIT() { bridge.reset(); }
