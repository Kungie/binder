"""Binder VPN's sign-in service.

Friends sign in with a user name and a password. A right answer gets a one-time Tailscale auth key tagged
tag:friend, which the friend's Tailscale app signs in to the owner's tailnet with. The owner's policy lets that tag
reach the internet through exit nodes and nothing else.

    server.py serve                      HTTP on port 8080: POST /vpn/api/login, GET /vpn/api/health
    server.py users | status             JSON
    server.py add NAME | password NAME   the password on stdin
    server.py remove NAME                also deletes the user's devices from the tailnet
    server.py oauth                      the OAuth client id and secret on stdin, one per line

Standard library only. Data lives in $BINDER_VPN_DATA (default /data), files mode 0600.
"""

import hashlib
import hmac
import json
import os
import re
import secrets
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

NAME = re.compile(r"^[a-z0-9][a-z0-9-]{0,31}$")
TAG = "tag:friend"
WINDOW = 15 * 60
# Wrong answers in a window before sign-ins are refused: from one address, for one user, and from everyone.
TRIES = 5
USER_TRIES = 20
ALL_TRIES = 200
BODY = 4096
CONNECTIONS = 32
WRONG = "Wrong user name or password."
API = "https://api.tailscale.com/api/v2"


class Invalid(Exception):
    """A request Binder made that can't be done."""


class Refused(Exception):
    """A sign-in that is turned away."""


def hash_password(password, salt=None):
    salt = salt or secrets.token_bytes(16)
    digest = hashlib.scrypt(password.encode(), salt=salt, n=2**14, r=8, p=1, dklen=32)
    return salt.hex(), digest.hex()


def hostname(user):
    return f"friend-{user}"


class Tailscale:
    """Tailscale's API through an OAuth client."""

    def __init__(self, load_oauth):
        self.load_oauth = load_oauth

    def grant(self, id, secret):
        body = urllib.parse.urlencode({"client_id": id, "client_secret": secret}).encode()
        return self.request("POST", f"{API}/oauth/token", body, None, form=True)

    def token(self):
        oauth = self.load_oauth()
        if not oauth:
            raise Refused("Binder VPN isn't set up yet.")
        return self.grant(oauth["id"], oauth["secret"])["access_token"]

    def scopes(self, id, secret):
        """The scopes a client was given, read by asking for a token with it."""
        try:
            return self.grant(id, secret).get("scope", "")
        except urllib.error.HTTPError as e:
            if e.code in (400, 401, 403):
                raise Invalid("Tailscale didn't accept that client id and secret.") from e
            raise

    def request(self, method, url, body, token, form=False):
        req = urllib.request.Request(url, data=body, method=method)
        if token:
            req.add_header("Authorization", f"Bearer {token}")
        req.add_header("Content-Type", "application/x-www-form-urlencoded" if form else "application/json")
        with urllib.request.urlopen(req, timeout=15) as r:
            text = r.read().decode()
        return json.loads(text) if text else {}

    def create_key(self, user):
        body = {
            "capabilities": {"devices": {"create": {"reusable": False, "ephemeral": False, "preauthorized": True, "tags": [TAG]}}},
            "expirySeconds": 600,
            "description": f"binder-vpn {user}"[:50],
        }
        return self.request("POST", f"{API}/tailnet/-/keys", json.dumps(body).encode(), self.token())["key"]

    def list_devices(self):
        return self.request("GET", f"{API}/tailnet/-/devices", None, self.token()).get("devices", [])

    def delete_device(self, id):
        self.request("DELETE", f"{API}/device/{urllib.parse.quote(id, safe='')}", None, self.token())


class App:
    def __init__(self, data, tailscale=None, clock=time.time):
        self.data = data
        self.clock = clock
        self.tailscale = tailscale or Tailscale(self.oauth)
        self.lock = threading.Lock()
        self.failures = {}

    # Files

    def path(self, name):
        return os.path.join(self.data, name)

    def read(self, name, default):
        try:
            with open(self.path(name)) as f:
                return json.load(f)
        except FileNotFoundError:
            return default

    def write(self, name, value):
        tmp = self.path(f".{name}.tmp")
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w") as f:
            json.dump(value, f, indent=1)
        os.replace(tmp, self.path(name))

    def oauth(self):
        return self.read("oauth.json", None)

    def set_oauth(self, id, secret):
        if not id or not secret:
            raise Invalid("Both the client id and the secret are needed.")
        scopes = set(self.tailscale.scopes(id, secret).split())
        if "all" not in scopes:
            if "auth_keys" not in scopes:
                raise Invalid("This client can't make sign-in keys. Make one with Keys, Auth Keys (write, tag:friend).")
            if "devices:core" not in scopes:
                raise Invalid("This client can't remove devices. Make one with Devices, Core (write, tag:friend).")
        self.write("oauth.json", {"id": id, "secret": secret})

    # Users

    def all(self):
        return self.read("users.json", {})

    def users(self):
        return [{"name": n, "created": u["created"], "last_login": u.get("last_login")} for n, u in sorted(self.all().items())]

    @staticmethod
    def check(name, password=None):
        if not NAME.match(name or ""):
            raise Invalid("A name is 1 to 32 lowercase letters, digits or dashes.")
        if password is not None and len(password) < 8:
            raise Invalid("A password is at least 8 characters.")

    def add(self, name, password):
        self.check(name, password)
        with self.lock:
            users = self.all()
            if name in users:
                raise Invalid(f"There is already a user named {name}.")
            salt, digest = hash_password(password)
            users[name] = {"salt": salt, "hash": digest, "created": int(self.clock())}
            self.write("users.json", users)

    def password(self, name, password):
        self.check(name, password)
        with self.lock:
            users = self.all()
            if name not in users:
                raise Invalid(f"There is no user named {name}.")
            users[name]["salt"], users[name]["hash"] = hash_password(password)
            self.write("users.json", users)

    def remove(self, name):
        self.check(name)
        mine = re.compile(rf"^{re.escape(hostname(name))}(-\d+)?$")
        try:
            devices = self.tailscale.list_devices()
        except Refused:  # Not set up, so nothing of theirs can be on the tailnet.
            devices = []
        for d in devices:
            host = d.get("hostname") or d.get("name", "").split(".")[0]
            if TAG in (d.get("tags") or []) and mine.match(host):
                self.tailscale.delete_device(d["id"])
        with self.lock:
            users = self.all()
            users.pop(name, None)
            self.write("users.json", users)

    # Sign-in

    def locked(self, key, now, limit):
        recent = [t for t in self.failures.get(key, []) if now - t < WINDOW]
        if recent:
            self.failures[key] = recent
        else:
            self.failures.pop(key, None)
        return len(recent) >= limit

    def login(self, name, password, address):
        now = self.clock()
        keys = [(f"ip:{address}", TRIES), (f"user:{name}", USER_TRIES), ("all", ALL_TRIES)]
        with self.lock:
            if any(self.locked(k, now, limit) for k, limit in keys):
                raise Refused("Too many tries. Wait 15 minutes.")
            user = self.all().get(name) if NAME.match(name or "") else None
            # Hash even for an unknown name, so the answer takes as long either way.
            salt = bytes.fromhex(user["salt"]) if user else b"\0" * 16
            digest = hash_password(password or "", salt)[1]
            if not user or not hmac.compare_digest(digest, user["hash"]):
                for k, _ in keys:
                    self.failures.setdefault(k, []).append(now)
                raise Refused(WRONG)
        key = self.tailscale.create_key(name)
        with self.lock:
            users = self.all()
            if name in users:
                users[name]["last_login"] = int(now)
                self.write("users.json", users)
        return {"key": key, "hostname": hostname(name)}


class Handler(BaseHTTPRequestHandler):
    app = None
    # A client that sends nothing for this long is dropped, so slow clients can't hold threads.
    timeout = 10
    server_version = "binder-vpn"
    sys_version = ""
    slots = threading.BoundedSemaphore(CONNECTIONS)

    def handle(self):
        if not Handler.slots.acquire(blocking=False):
            try:
                self.answer_raw(503, {"error": "Busy. Try again in a minute."})
            except OSError:
                pass
            return
        try:
            super().handle()
        finally:
            Handler.slots.release()

    def answer_raw(self, code, body):
        data = json.dumps(body).encode()
        self.wfile.write(f"HTTP/1.1 {code} Busy\r\nContent-Type: application/json\r\nContent-Length: {len(data)}\r\nConnection: close\r\n\r\n".encode() + data)

    def answer(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def address(self):
        """The client's address, as the proxy in front of the service gives it: Cloudflare's header, else the last
        X-Forwarded-For entry (the one the proxy added), else the connection's own."""
        cf = (self.headers.get("Cf-Connecting-Ip") or "").strip()
        if cf:
            return cf[:64]
        forwarded = [a.strip() for a in (self.headers.get("X-Forwarded-For") or "").split(",") if a.strip()]
        return forwarded[-1][:64] if forwarded else self.client_address[0]

    def do_GET(self):
        if self.path == "/vpn/api/health":
            return self.answer(200, {"ok": True, "configured": bool(self.app.oauth())})
        self.answer(404, {"error": "Not found."})

    def do_POST(self):
        if self.path != "/vpn/api/login":
            return self.answer(404, {"error": "Not found."})
        try:
            length = int(self.headers.get("Content-Length", ""))
        except ValueError:
            return self.answer(400, {"error": "That isn't a sign-in."})
        if length < 0:
            return self.answer(400, {"error": "That isn't a sign-in."})
        if length > BODY:
            self.close_connection = True
            return self.answer(413, {"error": "That isn't a sign-in."})
        try:
            body = json.loads(self.rfile.read(length) or b"{}")
            if not isinstance(body, dict):
                raise ValueError
            user, password = body.get("user", ""), body.get("password", "")
            if not isinstance(user, str) or not isinstance(password, str) or len(user) > 64 or len(password) > 256:
                raise ValueError
        except (ValueError, UnicodeDecodeError):
            return self.answer(400, {"error": "That isn't a sign-in."})
        try:
            self.answer(200, self.app.login(user.strip().lower(), password, self.address()))
        except Refused as e:
            self.answer(401 if str(e) == WRONG else 429 if "Too many" in str(e) else 503, {"error": str(e)})
        except Exception as e:  # Tailscale's API failed.
            sys.stderr.write(f"login failed at Tailscale: {type(e).__name__}\n")
            self.answer(502, {"error": "Tailscale didn't give a key. Try again in a minute."})

    def log_message(self, format, *args):
        # The request line and status only: no bodies, no passwords, no keys.
        sys.stderr.write(f"{self.address()} {self.command} {self.path[:80]} {args[1] if len(args) > 1 else ''}\n")


class Server(ThreadingHTTPServer):
    daemon_threads = True
    block_on_close = False
    request_queue_size = 64

    def server_bind(self):
        # HTTPServer looks up its own host name here, which can stall for half a minute; nothing uses it.
        import socketserver

        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


def make_server(address):
    return Server(address, Handler)


def cli(app, args, stdin, out):
    try:
        cmd, rest = (args[0], args[1:]) if args else ("", [])
        if cmd == "users":
            json.dump(app.users(), out)
        elif cmd == "status":
            json.dump({"configured": bool(app.oauth()), "users": len(app.all())}, out)
        elif cmd in ("add", "password") and len(rest) == 1:
            password = stdin.readline().rstrip("\n")
            (app.add if cmd == "add" else app.password)(rest[0], password)
        elif cmd == "remove" and len(rest) == 1:
            app.remove(rest[0])
        elif cmd == "oauth":
            app.set_oauth(stdin.readline().strip(), stdin.readline().strip())
        else:
            raise Invalid("Usage: users | status | add NAME | password NAME | remove NAME | oauth | serve")
        return 0
    except (Invalid, Refused) as e:
        out.write(str(e))
        return 1
    except Exception as e:  # Tailscale's API failed.
        out.write(f"Tailscale's API failed: {e}")
        return 1


def main():
    app = App(os.environ.get("BINDER_VPN_DATA", "/data"))
    if sys.argv[1:] == ["serve"]:
        Handler.app = app
        make_server(("0.0.0.0", 8080)).serve_forever()
    import io

    out = io.StringIO()
    code = cli(app, sys.argv[1:], sys.stdin, out)
    # Errors go to stderr, which is what Binder shows when the command fails.
    (sys.stdout if code == 0 else sys.stderr).write(out.getvalue() + "\n")
    sys.exit(code)


if __name__ == "__main__":
    main()
