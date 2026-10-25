#pragma once

#include <algorithm>
#include <cerrno>
#include <cstring>
#include <deque>
#include <string>
#include <vector>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

namespace writy {
// Main-loop only. Never wait for the daemon or retain input after a lost connection.
class Transport {
  public:
    explicit Transport(std::string path, size_t queueLimit = 2 * 1024 * 1024)
        : path_(std::move(path)), queueLimit_(queueLimit) {}
    ~Transport() { disconnect(); }
    Transport(const Transport&) = delete;
    Transport& operator=(const Transport&) = delete;

    bool connected() const { return fd_ >= 0; }
    bool poll() {
        if (connected() || path_.size() >= sizeof(sockaddr_un::sun_path)) return false;
        fd_ = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
        if (fd_ < 0) return false;
        sockaddr_un address{};
        address.sun_family = AF_UNIX;
        std::memcpy(address.sun_path, path_.c_str(), path_.size() + 1);
        if (connect(fd_, reinterpret_cast<sockaddr*>(&address), sizeof(address)) < 0) {
            disconnect();
            return false;
        }
        ucred credentials{};
        socklen_t length = sizeof(credentials);
        if (getsockopt(fd_, SOL_SOCKET, SO_PEERCRED, &credentials, &length) < 0 || credentials.uid != geteuid()) {
            disconnect();
            return false;
        }
        return true;
    }
    bool enqueue(std::string line) {
        if (!connected()) return false;
        line.push_back('\n');
        if (line.size() > queueLimit_ || queued_ > queueLimit_ - line.size()) {
            disconnect();
            return false;
        }
        queued_ += line.size();
        outgoing_.push_back(std::move(line));
        return true;
    }
    void flush() {
        size_t budget = 65536;
        while (connected() && !outgoing_.empty() && budget != 0) {
            const auto& line = outgoing_.front();
            const auto count = send(fd_, line.data() + offset_, std::min(budget, line.size() - offset_), MSG_DONTWAIT | MSG_NOSIGNAL);
            if (count < 0 && errno == EINTR) continue;
            if (count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) return;
            if (count <= 0) { disconnect(); return; }
            offset_ += count;
            queued_ -= count;
            budget -= count;
            if (offset_ == line.size()) { outgoing_.pop_front(); offset_ = 0; }
        }
    }
    std::vector<std::string> readLines() {
        std::vector<std::string> result;
        char buffer[4096];
        // Bound work even if a broken daemon writes forever.
        for (unsigned iteration = 0; connected() && iteration < 4; ++iteration) {
            const auto count = recv(fd_, buffer, sizeof(buffer), MSG_DONTWAIT);
            if (count < 0 && errno == EINTR) continue;
            if (count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) break;
            if (count <= 0) { disconnect(); break; }
            incoming_.append(buffer, count);
            size_t end;
            while ((end = incoming_.find('\n')) != std::string::npos) {
                if (end > 4096) { disconnect(); return {}; }
                result.push_back(incoming_.substr(0, end));
                incoming_.erase(0, end + 1);
            }
            if (incoming_.size() > 4096) { disconnect(); return {}; }
        }
        return result;
    }
    void disconnect() {
        if (fd_ >= 0) close(fd_);
        fd_ = -1;
        outgoing_.clear(); incoming_.clear();
        offset_ = queued_ = 0;
    }
  private:
    std::string path_, incoming_;
    size_t queueLimit_, offset_ = 0, queued_ = 0;
    int fd_ = -1;
    std::deque<std::string> outgoing_;
};
}
