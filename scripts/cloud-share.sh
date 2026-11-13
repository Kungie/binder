#!/usr/bin/env bash
# Sets up Binder's Cloud share on the home server: /srv/cloud over SMB, next to the vault share.
#
#   sudo ./cloud-share.sh <user>
#
# - Creates /srv/cloud and /srv/cloud/.trash, owned by <user>, mode 0750.
# - Adds a [cloud] share with the vault's settings (SMB3 and network limits come from [global]).
# - Adds the macOS modules to [vault] and [cloud], so Finder stops leaving ._ files behind.
# - Checks the config with testparm, puts the old one back if it fails, then reloads Samba.
# Safe to run more than once. To move Cloud to another disk later: copy the data, change `path` under
# [cloud] in /etc/samba/smb.conf, and reload Samba.
set -euo pipefail

user="${1:-}"
if [[ -z "$user" ]]; then
  echo "usage: sudo $0 <user>" >&2
  exit 2
fi
if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run it with sudo." >&2
  exit 1
fi
if ! id "$user" >/dev/null 2>&1; then
  echo "No user called $user." >&2
  exit 1
fi

conf=/etc/samba/smb.conf
root=/srv/cloud
backup="$conf.before-binder-cloud"

install -d -m 0750 -o "$user" -g "$user" "$root" "$root/.trash"

cp -p "$conf" "$backup"

# Sets `key = value` inside [section], replacing the key if it's there and adding it at the section's end if not.
ensure() {
  local section="$1" key="$2" value="$3" tmp
  tmp="$(mktemp)"
  # Blank lines are held back, so a new key lands right after the last setting of its section.
  awk -v sec="[$section]" -v key="$key" -v val="$value" '
    function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t]+$/, "", s); return s }
    function flush() { if (insec && !seen) print "   " key " = " val; insec = 0 }
    function release() { printf "%s", blanks; blanks = "" }
    {
      t = trim($0)
      if (t == "") { blanks = blanks $0 "\n"; next }
      if (t ~ /^\[/) { flush(); release(); insec = (tolower(t) == tolower(sec)); seen = 0; print; next }
      release()
      if (insec && t !~ /^[#;]/ && index(t, "=") > 0) {
        k = tolower(trim(substr(t, 1, index(t, "=") - 1)))
        if (k == tolower(key)) { if (!seen) print "   " key " = " val; seen = 1; next }
      }
      print
    }
    END { flush(); release() }
  ' "$conf" >"$tmp"
  cat "$tmp" >"$conf"
  rm -f "$tmp"
}

if ! grep -qiE '^[[:space:]]*\[cloud\][[:space:]]*$' "$conf"; then
  printf '\n[cloud]\n' >>"$conf"
fi

ensure cloud path "$root"
ensure cloud "valid users" "$user"
ensure cloud "guest ok" no
ensure cloud browseable no
ensure cloud "read only" no
ensure cloud "smb encrypt" required
ensure cloud "create mask" 0640
ensure cloud "directory mask" 0750

for share in vault cloud; do
  if grep -qiE "^[[:space:]]*\[$share\][[:space:]]*$" "$conf"; then
    ensure "$share" "vfs objects" "catia fruit streams_xattr"
    ensure "$share" "fruit:metadata" stream
    ensure "$share" "fruit:resource" stream
  fi
done

if ! testparm -s "$conf" >/dev/null 2>&1; then
  cp -p "$backup" "$conf"
  echo "testparm rejected the new config; the old one is back. Run testparm -s to see why." >&2
  exit 1
fi

systemctl reload smbd 2>/dev/null || smbcontrol all reload-config

echo "Cloud is shared as [cloud] from $root."
echo "On the Mac: Finder, Go, Connect to Server, smb://<server>/cloud, then set the same address in Binder's Settings."
echo "The previous config is in $backup."
