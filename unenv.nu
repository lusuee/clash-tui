# Unset Clash Proxy in Nushell
for v in ["http_proxy", "https_proxy", "all_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
    if $v in $env {
        hide-env $v
    }
}
print "✔ Proxy env cleared"
