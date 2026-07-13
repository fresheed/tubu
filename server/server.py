#!/usr/bin/env python3
"""Simple HTTP server for serving DASH content (manifest.mpd + .m4s segments)."""
import argparse
import http.server
import os
import socketserver
import sys

DEFAULT_PORT = 8000
LOG_FILE = "server.log"
PID_FILE = "server.pid"

EXTRA_MIME_TYPES = {
    ".mpd": "application/dash+xml",
    ".m4s": "video/iso.segment",
    ".mp4": "video/mp4",
}


class DashServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True


class DashRequestHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()

    def guess_type(self, path):
        for ext, mime in EXTRA_MIME_TYPES.items():
            if path.endswith(ext):
                return mime
        return super().guess_type(path)


def daemonize():
    """Double-fork into a detached background process, redirecting
    stdout/stderr to LOG_FILE and recording the pid in PID_FILE."""
    if os.fork() > 0:
        sys.exit(0)
    os.setsid()
    if os.fork() > 0:
        sys.exit(0)

    log_fd = os.open(LOG_FILE, os.O_CREAT | os.O_WRONLY | os.O_APPEND, 0o644)
    os.dup2(log_fd, sys.stdout.fileno())
    os.dup2(log_fd, sys.stderr.fileno())

    with open(PID_FILE, "w") as f:
        f.write(str(os.getpid()))


def run(port):
    with DashServer(("", port), DashRequestHandler) as httpd:
        print(f"Serving on http://localhost:{port}  (open /index.html)", flush=True)
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("port", nargs="?", type=int, default=DEFAULT_PORT)
    parser.add_argument(
        "-d", "--daemon",
        action="store_true",
        help=f"run detached in the background (logs to {LOG_FILE}, pid in {PID_FILE}) "
             f"instead of serving in the foreground until Ctrl-C",
    )
    args = parser.parse_args()

    if args.daemon:
        daemonize()

    run(args.port)
