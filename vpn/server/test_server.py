import io
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
            {"id": "1", "hostname": "friend-ali", "tags": ["tag:friend"]},
            {"id": "2", "hostname": "friend-ali-1", "tags": ["tag:friend"]},
            {"id": "3", "hostname": "friend-ali-b", "tags": ["tag:friend"]},
            {"id": "4", "hostname": "friend-ali", "tags": []},
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
        self.app.add("ali", "long enough")
        self.assertEqual(self.app.login("ali", "long enough", "1.2.3.4"), {"key": "tskey-auth-test", "hostname": "friend-ali"})
        self.assertEqual(self.ts.keys, ["ali"])
        self.assertEqual(self.app.users()[0]["last_login"], 1000)

    def test_hash_is_stored_not_the_password(self):
        self.app.add("ali", "long enough")
        text = open(os.path.join(self.dir, "users.json")).read()
        self.assertNotIn("long enough", text)
        self.assertEqual(os.stat(os.path.join(self.dir, "users.json")).st_mode & 0o777, 0o600)

    def test_wrong_password_is_one_message(self):
        self.app.add("ali", "long enough")
        with self.assertRaisesRegex(server.Refused, "^Wrong user name or password.$"):
            self.app.login("ali", "nope nope", "1.2.3.4")
        with self.assertRaisesRegex(server.Refused, "^Wrong user name or password.$"):
            self.app.login("nobody", "nope nope", "1.2.3.4")
        self.assertEqual(self.ts.keys, [])

    def test_names_and_passwords_are_checked(self):
        for bad in ["", "Ali", "-ali", "a" * 33, "ali b", "ali/../x"]:
            with self.assertRaises(server.Invalid):
                self.app.add(bad, "long enough")
        with self.assertRaises(server.Invalid):
            self.app.add("ali", "short")
        self.app.add("ali", "long enough")
        with self.assertRaises(server.Invalid):
            self.app.add("ali", "long enough")

    def test_password_change(self):
        self.app.add("ali", "long enough")
        self.app.password("ali", "another one")
        self.assertEqual(self.app.login("ali", "another one", "x")["hostname"], "friend-ali")
        with self.assertRaises(server.Invalid):
            self.app.password("nobody", "another one")

    def test_remove_deletes_only_that_users_tagged_devices(self):
        self.app.add("ali", "long enough")
        self.app.remove("ali")
        self.assertEqual(self.ts.deleted, ["1", "2"])
        self.assertEqual(self.app.users(), [])


class Limits(Base):
    def test_five_failures_from_an_address_lock_it_for_the_window(self):
        self.app.add("ali", "long enough")
        for _ in range(5):
            with self.assertRaises(server.Refused):
                self.app.login("ali", "wrong pass", "1.2.3.4")
        with self.assertRaisesRegex(server.Refused, "Too many tries"):
            self.app.login("ali", "long enough", "1.2.3.4")
        self.now += 15 * 60 + 1
        self.assertEqual(self.app.login("ali", "long enough", "1.2.3.4")["hostname"], "friend-ali")

    def test_five_failures_for_a_user_lock_the_user(self):
        self.app.add("ali", "long enough")
        for i in range(5):
            with self.assertRaises(server.Refused):
                self.app.login("ali", "wrong pass", f"10.0.0.{i}")
        with self.assertRaisesRegex(server.Refused, "Too many tries"):
            self.app.login("ali", "long enough", "10.0.0.9")


class Cli(Base):
    def run_cli(self, args, stdin=""):
        out = io.StringIO()
        code = server.cli(self.app, args, io.StringIO(stdin), out)
        return code, out.getvalue()

    def test_add_list_remove(self):
        self.assertEqual(self.run_cli(["add", "ali"], "long enough\n")[0], 0)
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
        code, out = self.run_cli(["add", "Bad"], "long enough\n")
        self.assertEqual(code, 1)
        self.assertIn("name", out)


if __name__ == "__main__":
    unittest.main()
