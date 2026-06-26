#!/usr/bin/env bash
# Generate libFuzzer seed corpora under fuzz/corpus/<target>/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORPUS="$ROOT/fuzz/corpus"

log() {
    printf 'mk-fuzz-corpus: %s\n' "$*" >&2
}

write_seed() {
    local target=$1
    local name=$2
    local dir="$CORPUS/$target"
    mkdir -p "$dir"
    cat >"$dir/$name"
}

main() {
    log "writing corpora to $CORPUS"

    # --- rash_parse: syntax coverage (parse only) ---
    write_seed rash_parse echo_hello <<'EOF'
echo hello world
EOF
    write_seed rash_parse pipeline <<'EOF'
echo hi | wc -c
EOF
    write_seed rash_parse if_then <<'EOF'
if true; then echo yes; fi
EOF
    write_seed rash_parse if_elif_else <<'EOF'
if false; then echo no; elif true; then echo elif; else echo else; fi
EOF
    write_seed rash_parse while_loop <<'EOF'
while false; do echo never; done
EOF
    write_seed rash_parse for_in_list <<'EOF'
for x in a b c; do echo $x; done
EOF
    write_seed rash_parse for_empty_in <<'EOF'
for x in; do echo never; done
EOF
    write_seed rash_parse for_without_in <<'EOF'
for x; do echo $x; done
EOF
    write_seed rash_parse function_def <<'EOF'
f() { echo hi; return 0; }; f
EOF
    write_seed rash_parse case_glob <<'EOF'
case foo in f*) echo match ;; *) echo nomatch ;; esac
EOF
    write_seed rash_parse heredoc <<'SEED'
cat <<EOF
hello world
EOF
SEED
    write_seed rash_parse redirects <<'EOF'
echo out > /tmp/out.txt 2> /tmp/err.txt
EOF
    write_seed rash_parse subshell <<'EOF'
(X=inner); echo $X
EOF
    write_seed rash_parse quotes <<'EOF'
echo 'no $expansion' "yes $HOME" $(echo sub)
EOF
    write_seed rash_parse background <<'EOF'
sleep 1 &
EOF

    # --- rash_arith: arithmetic expressions ---
    write_seed rash_arith add_mul <<'EOF'
1+2*3
EOF
    write_seed rash_arith modulo <<'EOF'
10 % 3
EOF
    write_seed rash_arith unary_parens <<'EOF'
-(2 + 3)
EOF
    write_seed rash_arith variable <<'EOF'
i+1
EOF
    write_seed rash_arith nested_parens <<'EOF'
((1+2)*(3+4))
EOF
    write_seed rash_arith divide <<'EOF'
100 / 5 - 2
EOF

    # --- rash_run: executable shell scripts (builtins only; PATH cleared in fuzz harness) ---
    write_seed rash_run echo_basic <<'EOF'
echo rash fuzz seed
EOF
    write_seed rash_run variables <<'EOF'
FOO=bar
echo $FOO
EOF
    write_seed rash_run export_child <<'EOF'
export X=exported
echo $X
EOF
    write_seed rash_run command_substitution <<'EOF'
echo $(echo substituted)
EOF
    write_seed rash_run cmdsub_assignment <<'EOF'
X=$(echo assigned)
echo $X
EOF
    write_seed rash_run cmdsub_double_quotes <<'EOF'
echo "wrap $(echo inside)"
EOF
    write_seed rash_run single_quotes <<'EOF'
echo 'literal $(no expansion)'
EOF
    write_seed rash_run for_empty_in <<'EOF'
for x in; do exit 9; done
echo done
EOF
    write_seed rash_run for_positional <<'EOF'
set -- one two
for x; do echo $x; done
EOF
    write_seed rash_run pipeline_builtin <<'EOF'
echo hello | echo world
EOF
    write_seed rash_run test_bracket <<'EOF'
[ -z "" ] && echo empty
EOF
    write_seed rash_run set_options <<'EOF'
set -e
false || true
set -o pipefail
true | false
echo $?
EOF
    write_seed rash_run function_local <<'EOF'
f() { local x=inner; echo $x; return 5; }
f
echo $?
EOF
    write_seed rash_run continue_break <<'EOF'
n=0
while [ $n -lt 3 ]; do
  n=$((n+1))
  if [ $n -eq 2 ]; then continue; fi
  if [ $n -eq 3 ]; then break; fi
  echo $n
done
EOF
    write_seed rash_run elif_else <<'EOF'
if false; then echo no; elif true; then echo elif; else echo else; fi
EOF
    write_seed rash_run case_glob <<'EOF'
case foo in f*) echo match ;; *) echo nomatch ;; esac
EOF
    write_seed rash_run heredoc <<'SEED'
read -r text <<'BODY'
hello fuzz
BODY
echo $text
SEED
    write_seed rash_run function_keyword <<'EOF'
function greet { echo hi; }
greet
EOF
    write_seed rash_run brace_group <<'EOF'
{ echo one; echo two; }
EOF
    write_seed rash_run subshell <<'EOF'
(X=inner); echo $X
EOF
    write_seed rash_run set_xtrace <<'EOF'
set -x
echo traced
set +x
EOF
    write_seed rash_run background <<'EOF'
: &
wait
EOF
    write_seed rash_run negated_pipeline <<'EOF'
! false
echo $?
EOF
    write_seed rash_run redirects <<'EOF'
echo out > /dev/null 2> /dev/null
echo append >> /dev/null
EOF
    write_seed rash_run redirect_only <<'EOF'
> /dev/null
EOF
    write_seed rash_run trap_list <<'EOF'
trap
trap - INT
EOF
    write_seed rash_run eval_script <<'EOF'
eval 'echo evaluated'
EOF
    write_seed rash_run and_or_chain <<'EOF'
false || echo or
true && echo and
EOF
    write_seed rash_run for_in_words <<'EOF'
for w in one two three; do echo $w; done
EOF
    write_seed rash_run pipe_three <<'EOF'
echo a | echo b | echo c
EOF
    write_seed rash_run shift_unset <<'EOF'
set -- a b c
shift
echo $1
unset FOO
FOO=1
unset FOO
echo done
EOF
    write_seed rash_run read_here <<'SEED'
read -r line <<'IN'
seed line
IN
echo $line
SEED

    # --- udhcpc: argv strings and minimal DHCP-shaped bytes ---
    write_seed udhcpc argv_basic <<'EOF'
-i eth0 -q -n -t 5
EOF
    write_seed udhcpc argv_short_flags <<'EOF'
-qn -i lo -T 2
EOF
    write_seed udhcpc iface_only <<'EOF'
eth0
EOF
    write_seed udhcpc help <<'EOF'
-h
EOF
    # BOOTREQUEST-like header (240 bytes) + empty options
    python3 - "$CORPUS/udhcpc/dhcp_min_bootrequest.bin" <<'PY'
import struct
import sys

path = sys.argv[1]
# op, htype, hlen, hops, xid, secs, flags, ciaddr, yiaddr, siaddr, giaddr, chaddr[16], sname[64], file[128]
pkt = bytearray(240)
pkt[0] = 1  # BOOTREQUEST
pkt[1] = 1  # ethernet
pkt[2] = 6  # hlen
struct.pack_into("!I", pkt, 4, 0x12345678)  # xid
pkt[236:240] = bytes([99, 130, 83, 99])  # magic cookie
pkt += bytes([255])  # end option
open(path, "wb").write(pkt)
PY

    # --- thttpd: argv, config, HTTP request lines ---
    write_seed thttpd argv_foreground <<'EOF'
-f -p 8080 -d /var/www
EOF
    write_seed thttpd argv_config <<'EOF'
-c /etc/thttpd.conf
EOF
    write_seed thttpd config_template <<'EOF'
# thttpd configuration
port=80
dir=/var/www
cgidir=/var/www/cgi-bin
user=http
EOF
    write_seed thttpd http_get_root <<'EOF'
GET / HTTP/1.0
Host: localhost

EOF
    write_seed thttpd http_get_cgi <<'EOF'
GET /cgi-bin/smoke-cgi HTTP/1.1
Host: 127.0.0.1
Connection: close

EOF
    write_seed thttpd http_path_traversal <<'EOF'
GET /../etc/passwd HTTP/1.0

EOF

    # --- wget: argv and URLs ---
    write_seed wget argv_basic <<'EOF'
-q -O - http://127.0.0.1/
EOF
    write_seed wget url_http <<'EOF'
http://example.com/path?a=1&b=2
EOF
    write_seed wget url_host_port <<'EOF'
http://localhost:8080/index.html
EOF
    write_seed wget http_response <<'EOF'
HTTP/1.1 200 OK
Content-Type: text/plain
Content-Length: 5

hello
EOF

    # --- dnscached: argv and config ---
    write_seed dnscached argv_basic <<'EOF'
-f -c /etc/dnscached.conf -l 127.0.0.1 -p 53
EOF
    write_seed dnscached config_template <<'EOF'
# dnscached configuration
upstream 8.8.8.8
upstream 8.8.4.4
host dns.google
path /dns-query
listen 0.0.0.0
port 53
user dnscache
EOF
    # minimal DNS query for example.com A
    python3 - "$CORPUS/dnscached/query_example_com.bin" <<'PY'
import sys

path = sys.argv[1]
# id=1, flags=standard query, qdcount=1, question example.com A
query = bytes.fromhex(
    "0001 0100 0001 0000 0000 0000"
    "076578616d706c6503636f6d0000010001".replace(" ", "")
)
open(path, "wb").write(query)
PY

    # --- sshd: argv and config ---
    write_seed sshd argv_basic <<'EOF'
-f -c /etc/sshd.conf -l 0.0.0.0 -p 22
EOF
    write_seed sshd config_template <<'EOF'
# Dev-only sshd. Do not expose to untrusted networks.
listen 0.0.0.0
port 22
passwd /etc/passwd
hostkey /etc/sshd_host_key
EOF
    write_seed sshd passwd_line <<'EOF'
root:$2b$12$gJ4YRl4nd9asLPOOk8Il2O1cbgwFwh./xyVQOxVfdtJ6kmKHmcPVW:0:0:root:/root:/bin/rash
EOF

    local total
    total="$(find "$CORPUS" -type f | wc -l)"
    log "wrote $total seed files"
    for target in rash_parse rash_arith rash_run udhcpc thttpd wget dnscached sshd; do
        local count
        count="$(find "$CORPUS/$target" -type f 2>/dev/null | wc -l)"
        log "  $target: $count seeds"
    done
    log "run: cd fuzz && cargo +nightly fuzz run rash_parse fuzz/corpus/rash_parse"
}

main "$@"
