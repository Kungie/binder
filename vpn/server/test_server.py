import io
import socket
import threading
import time
import json
import os
import tempfile
import unittest

import server


class FakeTailscale:
    def __init__(self):
        self.keys = []
        self.deleted = []
        self.devices = [
            {"id": "1", "nodeId": "n1", "hostname": "friend-ali", "tags": ["tag:friend"]},
            {"id": "2", "nodeId": "n2", "hostname": "friend-ali-1", "tags": ["tag:friend"]},
            {"id": "3", "nodeId": "n3", "hostname": "friend-ali-b", "tags": ["tag:friend"]},
            {"id": "4", "nodeId": "n4", "hostname": "friend-ali", "tags": []},
        ]

    scope = "auth_keys devices:core"

    def scopes(self, id, secret):
        if secret == "bad":
            raise server.Invalid("Tailscale didn't accept that client id and secret.")
        return self.scope

    def create_key(self, user):
        self.keys.append(user)
        return "tskey-auth-test"

    def list_devices(self):
        return self.devices

    def delete_device(self, id):
        self.deleted.append(id)


class Base(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.ts = FakeTailscale()
        self.app = server.App(self.dir, self.ts, clock=lambda: self.now)
        self.now = 1000.0


class Users(Base):
    def test_add_then_sign_in(self):
        self.app.add("ali", "long enough pw")
        answer = self.app.login("ali", "long enough pw", "1.2.3.4")
        self.assertEqual((answer["key"], answer["hostname"]), ("tskey-auth-test", "friend-ali"))
        self.assertGreaterEqual(len(answer["token"]), 40)
        self.assertEqual(self.ts.keys, ["ali"])
        self.assertEqual(self.app.users()[0]["last_login"], 1000)

    def test_hash_is_stored_not_the_password(self):
        self.app.add("ali", "long enough pw")
        with open(os.path.join(self.dir, "users.json")) as f:
            text = f.read()
        self.assertNotIn("long enough pw", text)
        self.assertEqual(os.stat(os.path.join(self.dir, "users.json")).st_mode & 0o777, 0o600)

    def test_wrong_password_is_one_message(self):
        self.app.add("ali", "long enough pw")
        with self.assertRaisesRegex(server.Refused, "^Wrong user name or password.$"):
            self.app.login("ali", "nope nope", "1.2.3.4")
        with self.assertRaisesRegex(server.Refused, "^Wrong user name or password.$"):
            self.app.login("nobody", "nope nope", "1.2.3.4")
        self.assertEqual(self.ts.keys, [])

    def test_names_and_passwords_are_checked(self):
        for bad in ["", "Ali", "-ali", "a" * 33, "ali b", "ali/../x"]:
            with self.assertRaises(server.Invalid):
                self.app.add(bad, "long enough pw")
        for short in ["short", "eleven char"]:
            with self.assertRaises(server.Invalid):
                self.app.add("ali", short)
        self.app.add("ali", "long enough pw")
        with self.assertRaises(server.Invalid):
            self.app.add("ali", "long enough pw")

    def test_password_change(self):
        self.app.add("ali", "long enough pw")
        self.app.password("ali", "another one pw")
        self.assertEqual(self.app.login("ali", "another one pw", "x")["hostname"], "friend-ali")
        with self.assertRaises(server.Invalid):
            self.app.password("nobody", "another one pw")

    def test_remove_deletes_only_that_users_tagged_devices(self):
        self.app.add("ali", "long enough pw")
        self.app.remove("ali")
        self.assertEqual(self.ts.deleted, ["1", "2"])
        self.assertEqual(self.app.users(), [])


class Limits(Base):
    def test_five_failures_from_an_address_lock_it_for_the_window(self):
        self.app.add("ali", "long enough pw")
        for _ in range(5):
            with self.assertRaises(server.Refused):
                self.app.login("ali", "wrong pass", "1.2.3.4")
        with self.assertRaisesRegex(server.Refused, "Too many tries"):
            self.app.login("ali", "long enough pw", "1.2.3.4")
        self.now += 15 * 60 + 1
        self.assertEqual(self.app.login("ali", "long enough pw", "1.2.3.4")["hostname"], "friend-ali")

    def test_failures_from_other_addresses_never_lock_a_friend_out(self):
        self.app.add("ali", "long enough pw")
        for i in range(60):
            with self.assertRaises(server.Refused):
                self.app.login("ali" if i % 2 else f"u{i}", "wrong pass", f"10.0.{i // 250}.{i % 250}")
        self.assertEqual(self.app.login("ali", "long enough pw", "10.9.9.9")["hostname"], "friend-ali")

    def test_ipv6_addresses_lock_by_their_64_prefix(self):
        self.app.add("ali", "long enough pw")
        for i in range(5):
            with self.assertRaises(server.Refused):
                self.app.login("ali", "wrong pass", f"2001:db8:1:2::{i + 1}")
        with self.assertRaisesRegex(server.Refused, "Too many tries"):
            self.app.login("ali", "long enough pw", "2001:db8:1:2:ffff::9")
        self.assertEqual(self.app.login("ali", "long enough pw", "2001:db8:1:3::1")["hostname"], "friend-ali")


class Logout(Base):
    def setUp(self):
        super().setUp()
        self.app.add("ali", "long enough pw")
        self.app.add("bob", "long enough pw")
        self.token = self.app.login("ali", "long enough pw", "1.2.3.4")["token"]

    def test_sign_out_deletes_that_device_only(self):
        self.app.logout("ali", self.token, "n2", "1.2.3.4")
        self.assertEqual(self.ts.deleted, ["2"])

    def test_the_token_works_once(self):
        self.app.logout("ali", self.token, "n1", "1.2.3.4")
        with self.assertRaisesRegex(server.Refused, "^Wrong user name or password.$"):
            self.app.logout("ali", self.token, "n1", "1.2.3.4")
        self.assertEqual(self.ts.deleted, ["1"])

    def test_the_token_is_stored_hashed(self):
        with open(os.path.join(self.dir, "users.json")) as f:
            self.assertNotIn(self.token, f.read())

    def test_a_wrong_token_or_another_users_device_deletes_nothing(self):
        with self.assertRaises(server.Refused):
            self.app.logout("ali", "x" * 43, "n1", "1.2.3.4")
        with self.assertRaises(server.Refused):
            self.app.logout("bob", self.token, "n1", "1.2.3.4")
        # Untagged, or not this user's name: the token is spent, nothing is deleted.
        self.app.logout("ali", self.token, "n4", "1.2.3.4")
        token = self.app.login("ali", "long enough pw", "1.2.3.4")["token"]
        self.app.logout("ali", token, "n3", "1.2.3.4")
        self.assertEqual(self.ts.deleted, [])

    def test_wrong_tokens_count_toward_the_lock(self):
        for _ in range(5):
            with self.assertRaises(server.Refused):
                self.app.logout("ali", "x" * 43, "n1", "6.6.6.6")
        with self.assertRaisesRegex(server.Refused, "Too many tries"):
            self.app.login("ali", "long enough pw", "6.6.6.6")

    def test_only_the_last_ten_sign_ins_keep_a_token(self):
        tokens = [self.app.login("ali", "long enough pw", "1.2.3.4")["token"] for _ in range(10)]
        with self.assertRaises(server.Refused):
            self.app.logout("ali", self.token, "n1", "1.2.3.4")
        self.app.logout("ali", tokens[0], "n1", "1.2.3.4")
        self.assertEqual(self.ts.deleted, ["1"])


class Http(Base):
    def setUp(self):
        super().setUp()
        self.now = time.time()
        self.app.clock = time.time
        self.app.add("ali", "long enough pw")
        server.Handler.app = self.app
        self.addCleanup(setattr, server.Handler, "log_message", server.Handler.log_message)
        server.Handler.log_message = lambda *a: None
        self.httpd = server.make_server(("127.0.0.1", 0))
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start()
        self.port = self.httpd.server_address[1]

    def tearDown(self):
        self.httpd.shutdown()
        self.httpd.server_close()

    def raw(self, request):
        with socket.create_connection(("127.0.0.1", self.port), timeout=5) as c:
            c.sendall(request)
            data = b""
            while chunk := c.recv(4096):
                data += chunk
        head, _, body = data.partition(b"\r\n\r\n")
        return int(head.split()[1]), head.decode(), body.decode()

    def post(self, body, extra=b"", path=b"/vpn/api/login"):
        return self.raw(b"POST " + path + b" HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Type: application/json\r\n" + extra + b"Content-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body)

    def test_sign_in_over_http(self):
        code, head, body = self.post(b'{"user":"ali","password":"long enough pw"}')
        self.assertEqual(code, 200)
        self.assertEqual(json.loads(body)["hostname"], "friend-ali")
        self.assertIn("no-store", head)
        self.assertNotIn("Python", head)

    def test_wrong_and_odd_requests(self):
        self.assertEqual(self.post(b'{"user":"ali","password":"nope nope"}')[0], 401)
        self.assertEqual(self.post(b"not json")[0], 400)
        self.assertEqual(self.post(b'["a"]')[0], 400)
        self.assertEqual(self.post(b"x" * 5000)[0], 413)
        code, _, _ = self.raw(b"POST /vpn/api/login HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: -1\r\n\r\n")
        self.assertEqual(code, 400)
        self.assertEqual(self.raw(b"GET /vpn/api/users HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")[0], 404)
        self.assertEqual(self.raw(b"GET /vpn/api/health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")[0], 200)

    def test_sign_out_over_http(self):
        token = json.loads(self.post(b'{"user":"ali","password":"long enough pw"}')[2])["token"]
        body = json.dumps({"user": "ali", "token": token, "node": "n1"}).encode()
        self.assertEqual(self.post(body, path=b"/vpn/api/logout")[0], 200)
        self.assertEqual(self.ts.deleted, ["1"])
        self.assertEqual(self.post(body, path=b"/vpn/api/logout")[0], 401)
        self.assertEqual(self.post(b'{"user":"ali","token":1,"node":"n1"}', path=b"/vpn/api/logout")[0], 400)

    def test_the_forwarded_address_is_the_one_locked(self):
        for _ in range(5):
            self.post(b'{"user":"ali","password":"nope nope"}', b"X-Forwarded-For: 1.1.1.1, 7.7.7.7\r\n")
        self.assertEqual(self.post(b'{"user":"ali","password":"long enough pw"}', b"X-Forwarded-For: 7.7.7.7\r\n")[0], 429)
        self.assertEqual(self.post(b'{"user":"ali","password":"long enough pw"}', b"X-Forwarded-For: 8.8.8.8\r\n")[0], 200)


class Cli(Base):
    def run_cli(self, args, stdin=""):
        out = io.StringIO()
        code = server.cli(self.app, args, io.StringIO(stdin), out)
        return code, out.getvalue()

    def test_add_list_remove(self):
        self.assertEqual(self.run_cli(["add", "ali"], "long enough pw\n")[0], 0)
        code, out = self.run_cli(["users"])
        self.assertEqual([u["name"] for u in json.loads(out)], ["ali"])
        self.assertNotIn("hash", out)
        self.assertEqual(self.run_cli(["remove", "ali"])[0], 0)

    def test_oauth_and_status(self):
        self.assertEqual(json.loads(self.run_cli(["status"])[1]), {"configured": False, "users": 0})
        self.assertEqual(self.run_cli(["oauth"], "id1\nsecret1\n")[0], 0)
        self.assertEqual(json.loads(self.run_cli(["status"])[1]), {"configured": True, "users": 0})
        self.assertNotIn("secret1", self.run_cli(["status"])[1])

    def test_a_client_without_auth_keys_is_refused_and_not_saved(self):
        self.ts.scope = "devices:core oauth_keys"
        code, out = self.run_cli(["oauth"], "id1\nsecret1\n")
        self.assertEqual(code, 1)
        self.assertIn("Auth Keys", out)
        self.assertEqual(json.loads(self.run_cli(["status"])[1])["configured"], False)
        self.ts.scope = "auth_keys"
        self.assertIn("Devices", self.run_cli(["oauth"], "id1\nsecret1\n")[1])
        self.assertEqual(self.run_cli(["oauth"], "id1\nbad\n")[0], 1)
        self.ts.scope = "all"
        self.assertEqual(self.run_cli(["oauth"], "id1\nsecret1\n")[0], 0)

    def test_errors_go_out_with_code_1(self):
        code, out = self.run_cli(["add", "Bad"], "long enough pw\n")
        self.assertEqual(code, 1)
        self.assertIn("name", out)


if __name__ == "__main__":
    unittest.main()
