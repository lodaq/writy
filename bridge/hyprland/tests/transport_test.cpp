#include "transport.hpp"
#include <cassert>
#include <filesystem>
#include <iostream>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

int main() {
    const auto path = "/tmp/writy-bridge-test-" + std::to_string(getpid()) + ".sock";
    writy::Transport transport(path, 128);
    assert(!transport.poll()); // Absence of Rust must never hold up the compositor.
    const int server = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    assert(server >= 0);
    sockaddr_un address{};
    address.sun_family = AF_UNIX;
    std::copy(path.begin(), path.end(), address.sun_path);
    assert(bind(server, reinterpret_cast<sockaddr*>(&address), sizeof(address)) == 0);
    assert(listen(server, 1) == 0);
    assert(transport.poll());
    int peer = accept4(server, nullptr, nullptr, SOCK_NONBLOCK | SOCK_CLOEXEC);
    assert(peer >= 0);
    assert(transport.enqueue("{\"type\":\"hello\"}"));
    transport.flush();
    char buffer[512]{};
    auto count = recv(peer, buffer, sizeof(buffer), 0);
    assert(std::string(buffer, count) == "{\"type\":\"hello\"}\n");
    assert(send(peer, "{\"type\":\"cap", 12, MSG_NOSIGNAL) == 12);
    assert(transport.readLines().empty());
    const std::string rest = "ture\"}\n{}\n";
    assert(send(peer, rest.data(), rest.size(), MSG_NOSIGNAL) == static_cast<ssize_t>(rest.size()));
    const auto lines = transport.readLines();
    assert(lines.size() == 2 && lines[0] == "{\"type\":\"capture\"}" && lines[1] == "{}");
    assert(!transport.enqueue(std::string(129, 'x')));
    assert(!transport.connected()); // Overflow discards the partial session; next hello resets Rust.
    close(peer);
    assert(transport.poll());
    peer = accept4(server, nullptr, nullptr, SOCK_NONBLOCK | SOCK_CLOEXEC);
    assert(peer >= 0);
    close(peer);
    transport.readLines();
    assert(!transport.connected());
    close(server);
    unlink(path.c_str());
    std::cout << "transport: reconnect, framing, overflow and disconnect passed\n";
}
