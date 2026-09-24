# Load Clash Proxy into Nushell
$env.http_proxy = "http://127.0.0.1:7897"
$env.https_proxy = "http://127.0.0.1:7897"
$env.all_proxy = "socks5://127.0.0.1:7897"
$env.HTTP_PROXY = "http://127.0.0.1:7897"
$env.HTTPS_PROXY = "http://127.0.0.1:7897"
$env.ALL_PROXY = "socks5://127.0.0.1:7897"
print $"✔ Proxy env loaded (7897)"
