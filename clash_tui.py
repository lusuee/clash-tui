"""
Clash TUI - Lightweight Clash/Mihomo Terminal Client
Built with Python 3 + Textual + httpx + pywin32 + PyYAML
"""

import argparse
import asyncio
import base64
import ctypes
import datetime
import json
import os
import random
import shutil
import subprocess
import sys
import time
from typing import Dict, List, Optional, Tuple
import urllib.parse
import winreg

import httpx
from rich.text import Text
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Container, Horizontal, Vertical
from textual.reactive import reactive
from textual.screen import ModalScreen
from textual.widgets import (
    Button,
    DataTable,
    Footer,
    Header,
    Input,
    Label,
    ListItem,
    ListView,
    Static,
    TabbedContent,
    TabPane,
)
import yaml


def format_bytes(b: float) -> str:
    kb = 1024.0
    mb = kb * 1024.0
    gb = mb * 1024.0
    if b >= gb:
        return f"{b / gb:.2f} GB"
    if b >= mb:
        return f"{b / mb:.1f} MB"
    if b >= kb:
        return f"{b / kb:.0f} KB"
    return f"{int(b)} B"


def format_speed(bps: float) -> str:
    return f"{format_bytes(bps)}/s"


# ==========================================
# Windows System Proxy Manager (WinINet)
# ==========================================
class SysProxyManager:
    INTERNET_OPTION_SETTINGS_CHANGED = 39
    INTERNET_OPTION_REFRESH = 37

    @classmethod
    def get_status(cls) -> Tuple[bool, str]:
        if sys.platform != "win32":
            return False, ""
        try:
            key_path = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings"
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key_path) as key:
                enabled, _ = winreg.QueryValueEx(key, "ProxyEnable")
                try:
                    server, _ = winreg.QueryValueEx(key, "ProxyServer")
                except FileNotFoundError:
                    server = ""
                return bool(enabled), server
        except Exception:
            return False, ""

    @classmethod
    def set_proxy(cls, enable: bool, server: str = "127.0.0.1:7890") -> bool:
        if sys.platform != "win32":
            return False
        try:
            key_path = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings"
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, key_path) as key:
                winreg.SetValueEx(key, "ProxyEnable", 0, winreg.REG_DWORD, 1 if enable else 0)
                if enable and server:
                    winreg.SetValueEx(key, "ProxyServer", 0, winreg.REG_SZ, server)

            # Notify Windows network subsystem of changes
            wininet = ctypes.windll.wininet
            wininet.InternetSetOptionW(0, cls.INTERNET_OPTION_SETTINGS_CHANGED, 0, 0)
            wininet.InternetSetOptionW(0, cls.INTERNET_OPTION_REFRESH, 0, 0)
            return True
        except Exception:
            return False


# ==========================================
# Mihomo Core Process Manager (Background Daemon)
# ==========================================
class CoreManager:
    def __init__(self, bin_path: str = "bin/mihomo.exe", data_dir: str = "data"):
        self.bin_path = os.path.abspath(bin_path)
        self.data_dir = os.path.abspath(data_dir)
        os.makedirs(os.path.dirname(self.bin_path), exist_ok=True)
        os.makedirs(self.data_dir, exist_ok=True)
        self.ensure_geodata()

    def is_installed(self) -> bool:
        return os.path.exists(self.bin_path)

    def ensure_geodata(self) -> None:
        # Check and copy local mmdb/dat files if available to prevent GitHub timeout
        target_files = ["Country.mmdb", "geoip.metadb", "geosite.dat", "geoip.dat"]
        missing = [f for f in target_files if not os.path.exists(os.path.join(self.data_dir, f))]
        if not missing:
            return

        candidate_dirs = [
            r"D:\Program Files\v2rayN-windows-64\bin",
            os.path.expandvars(r"%USERPROFILE%\.config\mihomo"),
        ]
        for src_dir in candidate_dirs:
            if os.path.exists(src_dir):
                for f in list(missing):
                    src_file = os.path.join(src_dir, f)
                    if os.path.exists(src_file):
                        try:
                            shutil.copyfile(src_file, os.path.join(self.data_dir, f))
                            missing.remove(f)
                        except Exception:
                            pass

    async def is_running(self, url: str = "http://127.0.0.1:9090", secret: str = "") -> bool:
        try:
            headers = {}
            if secret:
                headers["Authorization"] = f"Bearer {secret}"
            async with httpx.AsyncClient(timeout=0.6, trust_env=False, headers=headers) as client:
                r = await client.get(f"{url.rstrip('/')}/version")
                return r.status_code == 200
        except Exception:
            return False

    def activate_profile(self, profile_path: str) -> str:
        # Copy & ensure external-controller in data/config.yaml to satisfy SAFE_PATHS
        config_path = os.path.join(self.data_dir, "config.yaml")
        if os.path.exists(profile_path):
            try:
                with open(profile_path, "r", encoding="utf-8") as f_in:
                    content = f_in.read()
                data = yaml.safe_load(content)
                if isinstance(data, dict):
                    data["external-controller"] = "127.0.0.1:9090"
                    data.setdefault("mixed-port", 7890)
                    with open(config_path, "w", encoding="utf-8") as f_out:
                        yaml.safe_dump(data, f_out, allow_unicode=True)
                    return os.path.abspath(config_path)
            except Exception:
                shutil.copyfile(profile_path, config_path)
                return os.path.abspath(config_path)

        return self.ensure_default_config()

    def ensure_default_config(self) -> str:
        config_path = os.path.join(self.data_dir, "config.yaml")
        if not os.path.exists(config_path):
            with open(config_path, "w", encoding="utf-8") as f:
                f.write(
                    "mixed-port: 7890\n"
                    "allow-lan: false\n"
                    "mode: rule\n"
                    "log-level: info\n"
                    "external-controller: 127.0.0.1:9090\n"
                    "secret: ''\n"
                )
        return os.path.abspath(config_path)

    def start_core(self) -> Tuple[bool, str]:
        if not self.is_installed():
            return False, f"Core executable not found at {self.bin_path}"

        self.ensure_default_config()
        self.ensure_geodata()

        DETACHED_PROCESS = 0x00000008
        CREATE_NEW_PROCESS_GROUP = 0x00000200
        CREATE_NO_WINDOW = 0x08000000

        log_path = os.path.join(self.data_dir, "mihomo.log")
        log_file = open(log_path, "a", encoding="utf-8")

        flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
        try:
            proc = subprocess.Popen(
                [self.bin_path, "-d", self.data_dir],
                creationflags=flags,
                stdout=log_file,
                stderr=log_file,
                close_fds=True,
            )
            return True, f"Mihomo daemon started in background (PID {proc.pid})"
        except Exception as e:
            return False, f"Failed to start core: {e}"

    def stop_core(self) -> Tuple[bool, str]:
        try:
            subprocess.run(["taskkill", "/F", "/IM", "mihomo.exe"], capture_output=True, text=True)
            return True, "Stopped background Mihomo core processes"
        except Exception as e:
            return False, f"Failed to stop core: {e}"


# ==========================================
# Clash REST API Client
# ==========================================
class ClashAPI:
    def __init__(self, base_url: str = "http://127.0.0.1:9090", secret: str = ""):
        self.base_url = base_url.rstrip("/")
        self.secret = secret
        headers = {}
        if secret:
            headers["Authorization"] = f"Bearer {secret}"
        self.client = httpx.AsyncClient(base_url=self.base_url, headers=headers, timeout=10.0, trust_env=False)

    async def get_version(self) -> dict:
        r = await self.client.get("/version")
        r.raise_for_status()
        return r.json()

    async def get_configs(self) -> dict:
        r = await self.client.get("/configs")
        r.raise_for_status()
        return r.json()

    async def set_mode(self, mode: str) -> None:
        r = await self.client.patch("/configs", json={"mode": mode})
        r.raise_for_status()

    async def reload_config(self, config_path: str = "") -> None:
        payload = {"path": config_path} if config_path else {}
        r = await self.client.put("/configs?force=true", json=payload, timeout=15.0)
        r.raise_for_status()

    async def get_proxies(self) -> dict:
        r = await self.client.get("/proxies")
        r.raise_for_status()
        return r.json().get("proxies", {})

    async def select_proxy(self, group: str, node: str) -> None:
        encoded = urllib.parse.quote(group, safe="")
        r = await self.client.put(f"/proxies/{encoded}", json={"name": node})
        r.raise_for_status()

    async def test_delay(self, node: str, url: str = "http://www.gstatic.com/generate_204", timeout: int = 3000) -> int:
        encoded_node = urllib.parse.quote(node, safe="")
        params = {"timeout": timeout, "url": url}
        r = await self.client.get(f"/proxies/{encoded_node}/delay", params=params, timeout=timeout / 1000 + 1)
        r.raise_for_status()
        return r.json().get("delay", 0)

    async def get_connections(self) -> dict:
        r = await self.client.get("/connections")
        r.raise_for_status()
        return r.json()

    async def close_connection(self, conn_id: str) -> None:
        r = await self.client.delete(f"/connections/{conn_id}")
        r.raise_for_status()

    async def close_all_connections(self) -> None:
        r = await self.client.delete("/connections")
        r.raise_for_status()

    async def close(self):
        await self.client.aclose()


# ==========================================
# Subscription Manager
# ==========================================
class SubscriptionManager:
    def __init__(self, file_path: str = "subscriptions.json", profiles_dir: str = "profiles"):
        self.file_path = file_path
        self.profiles_dir = profiles_dir
        self.subscriptions: List[dict] = []
        os.makedirs(self.profiles_dir, exist_ok=True)
        self.load()

    def load(self) -> None:
        if os.path.exists(self.file_path):
            try:
                with open(self.file_path, "r", encoding="utf-8") as f:
                    self.subscriptions = json.load(f)
                    return
            except Exception:
                pass

        self.subscriptions = []
        self.save()

    def save(self) -> None:
        try:
            with open(self.file_path, "w", encoding="utf-8") as f:
                json.dump(self.subscriptions, f, ensure_ascii=False, indent=2)
        except Exception:
            pass

    def add(self, name: str, url: str) -> dict:
        sub_id = f"sub_{int(time.time())}"
        sub = {
            "id": sub_id,
            "name": name,
            "url": url,
            "active": len(self.subscriptions) == 0,
            "node_count": 0,
            "last_updated": "Never",
        }
        self.subscriptions.append(sub)
        self.save()
        return sub

    def delete(self, sub_id: str) -> None:
        self.subscriptions = [s for s in self.subscriptions if s["id"] != sub_id]
        if self.subscriptions and not any(s["active"] for s in self.subscriptions):
            self.subscriptions[0]["active"] = True
        self.save()

    def set_active(self, sub_id: str) -> Optional[dict]:
        active_sub = None
        for s in self.subscriptions:
            if s["id"] == sub_id:
                s["active"] = True
                active_sub = s
            else:
                s["active"] = False
        self.save()
        return active_sub

    def get_active(self) -> Optional[dict]:
        for s in self.subscriptions:
            if s.get("active"):
                return s
        return self.subscriptions[0] if self.subscriptions else None

    async def update_subscription(self, sub_id: str) -> Tuple[bool, str, int]:
        sub = next((s for s in self.subscriptions if s["id"] == sub_id), None)
        if not sub:
            return False, "Subscription not found", 0

        url = sub.get("url", "")
        if not url:
            return False, "Invalid URL", 0

        if "example.com" in url or "demo" in url:
            await asyncio.sleep(0.4)
            sub["node_count"] = random.randint(12, 35)
            sub["last_updated"] = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
            self.save()
            return True, f"Updated {sub['name']} ({sub['node_count']} nodes)", sub["node_count"]

        headers = {
            "User-Agent": "ClashMeta/1.18.0 (Windows NT 10.0; Win64; x64) ClashTUI/1.0"
        }
        try:
            async with httpx.AsyncClient(headers=headers, timeout=20.0, trust_env=False) as client:
                resp = await client.get(url, follow_redirects=True)
                resp.raise_for_status()
                content = resp.text

            node_count = self._parse_node_count(content)
            out_file = os.path.join(self.profiles_dir, f"{sub_id}.yaml")
            with open(out_file, "w", encoding="utf-8") as f:
                f.write(content)

            sub["node_count"] = node_count
            sub["last_updated"] = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
            self.save()
            return True, f"Updated {sub['name']}: {node_count} nodes found", node_count
        except Exception as e:
            return False, f"Failed to update {sub['name']}: {str(e)}", 0

    def _parse_node_count(self, content: str) -> int:
        try:
            data = yaml.safe_load(content)
            if isinstance(data, dict) and "proxies" in data:
                return len(data["proxies"])
        except Exception:
            pass

        try:
            decoded = base64.b64decode(content.strip()).decode("utf-8", errors="ignore")
            lines = [line.strip() for line in decoded.splitlines() if line.strip()]
            valid = [l for l in lines if "://" in l]
            if valid:
                return len(valid)
        except Exception:
            pass

        return 0


# ==========================================
# Mock Data Provider (For Fallback Demo)
# ==========================================
class MockClash:
    def __init__(self):
        self.groups = ["Proxy", "GLOBAL", "Auto - Fastest", "Domestic", "Others"]
        self.nodes = [
            ("HK 01 - IPLC [0.5x]", "Shadowsocks", 82),
            ("HK 02 - BGP [1.0x]", "VMess", 95),
            ("JP 01 - Tokyo High-Speed", "Hysteria2", 110),
            ("SG 01 - Singapore Direct", "Trojan", 135),
            ("US 01 - Silicon Valley", "VLESS", 185),
            ("US 02 - Los Angeles", "Shadowsocks", 204),
            ("DIRECT", "Direct", 15),
            ("REJECT", "Reject", 0),
        ]
        self.active_now = {g: self.nodes[0][0] for g in self.groups}
        self.mode = "Rule"
        self.mixed_port = 7890
        self.total_upload = 1024 * 1024 * 45
        self.total_download = 1024 * 1024 * 320

    def get_proxies(self) -> dict:
        result = {}
        node_names = [n[0] for n in self.nodes]
        for g in self.groups:
            result[g] = {
                "name": g,
                "type": "Selector",
                "now": self.active_now[g],
                "all": node_names,
            }
        for name, ptype, delay in self.nodes:
            result[name] = {
                "name": name,
                "type": ptype,
                "history": [{"delay": delay}],
            }
        return result

    def get_connections(self) -> dict:
        mock_hosts = [
            ("github.com:443", "MATCH", "Proxy", 1024 * 40, 1024 * 350),
            ("api.openai.com:443", "DOMAIN-SUFFIX", "Proxy", 1024 * 12, 1024 * 85),
            ("www.google.com:443", "DOMAIN-KEYWORD", "Proxy", 1024 * 8, 1024 * 120),
            ("bilibili.com:443", "GEOIP", "DIRECT", 1024 * 500, 1024 * 4200),
            ("cdn.jsdelivr.net:443", "DOMAIN-SUFFIX", "Proxy", 1024 * 5, 1024 * 45),
        ]
        conns = []
        for idx, (host, rule, chain, up, down) in enumerate(mock_hosts):
            conns.append({
                "id": f"mock-conn-{idx}",
                "metadata": {"host": host, "destinationIP": "104.26.1.1", "processPath": "chrome.exe"},
                "rule": rule,
                "chains": [chain],
                "upload": up,
                "download": down,
            })
        return {
            "uploadTotal": self.total_upload,
            "downloadTotal": self.total_download,
            "connections": conns,
        }


# ==========================================
# Modal Dialog: Add Subscription
# ==========================================
class AddSubscriptionModal(ModalScreen[Optional[Tuple[str, str]]]):
    CSS = """
    AddSubscriptionModal {
        align: center middle;
    }
    #dialog {
        width: 64;
        height: auto;
        border: thick #58a6ff;
        background: #161b22;
        padding: 1 2;
    }
    .dialog-title {
        text-style: bold;
        color: #58a6ff;
        margin-bottom: 1;
    }
    .field-label {
        color: #8b949e;
        margin-top: 1;
    }
    .dialog-buttons {
        margin-top: 1;
        layout: horizontal;
        align: right middle;
    }
    Button {
        margin-left: 1;
    }
    """

    def compose(self) -> ComposeResult:
        with Vertical(id="dialog"):
            yield Label("Add Subscription Profile", classes="dialog-title")
            yield Label("Subscription Name:", classes="field-label")
            yield Input(placeholder="e.g. My Fast Nodes", id="input_name")
            yield Label("Subscription URL (Clash YAML or Base64):", classes="field-label")
            yield Input(placeholder="https://example.com/api/v1/client/subscribe?token=...", id="input_url")
            with Horizontal(classes="dialog-buttons"):
                yield Button("Cancel", variant="default", id="btn_cancel")
                yield Button("Save & Add", variant="primary", id="btn_save")

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "btn_save":
            name = self.query_one("#input_name", Input).value.strip()
            url = self.query_one("#input_url", Input).value.strip()
            if name and url:
                self.dismiss((name, url))
            else:
                self.notify("Name and URL cannot be empty!", severity="warning")
        else:
            self.dismiss(None)


# ==========================================
# Main Textual TUI Application
# ==========================================
class ClashTUIApp(App):
    CSS = """
    Screen {
        background: #0f141c;
        color: #e6edf3;
    }

    #header_bar {
        dock: top;
        height: 3;
        background: #161b22;
        border-bottom: solid #30363d;
        padding: 0 1;
        layout: horizontal;
    }

    #logo {
        width: 16;
        text-style: bold;
        color: #58a6ff;
        padding-top: 1;
    }

    #status_badges {
        width: 1fr;
        layout: horizontal;
        padding-top: 1;
    }

    #traffic_box {
        width: 32;
        text-align: right;
        padding-top: 1;
        color: #7ee787;
    }

    TabbedContent {
        height: 1fr;
    }

    #proxies_container {
        height: 1fr;
        layout: horizontal;
    }

    #groups_pane {
        width: 30%;
        height: 1fr;
        border-right: solid #30363d;
    }

    #nodes_pane {
        width: 70%;
        height: 1fr;
    }

    #groups_table {
        height: 1fr;
    }

    #nodes_table {
        height: 1fr;
    }

    #connections_table {
        height: 1fr;
    }

    #subs_table {
        height: 1fr;
    }

    #settings_container {
        padding: 1 2;
    }

    .card {
        background: #161b22;
        border: solid #30363d;
        padding: 1 2;
        margin-bottom: 1;
    }

    .card-title {
        text-style: bold;
        color: #58a6ff;
        margin-bottom: 1;
    }

    #toast_bar {
        dock: bottom;
        height: 1;
        background: #1f242c;
        color: #d29922;
        padding: 0 1;
    }
    """

    BINDINGS = [
        Binding("q", "quit", "Quit"),
        Binding("tab", "next_tab", "Next Tab"),
        Binding("1", "tab_proxies", "Proxies"),
        Binding("2", "tab_conns", "Conns"),
        Binding("3", "tab_settings", "Settings"),
        Binding("4", "tab_subs", "Subscriptions"),
        Binding("m", "cycle_mode", "Mode"),
        Binding("p", "toggle_sys_proxy", "SysProxy"),
        Binding("t", "test_latency", "Ping"),
        Binding("T", "test_group_latency", "Ping All"),
        Binding("r", "refresh_data", "Refresh"),
        Binding("a", "add_sub", "Add Sub"),
        Binding("u", "update_sub", "Update Sub"),
        Binding("U", "update_all_subs", "Update All Subs"),
        Binding("x", "delete_sub", "Delete Sub"),
        Binding("k", "stop_core", "Stop Core"),
        Binding("K", "restart_core", "Restart Core"),
        Binding("d", "close_conn", "Close Conn"),
        Binding("D", "close_all_conns", "Close All"),
    ]

    # Reactive states
    is_online = reactive(False)
    mode = reactive("Rule")
    sys_proxy_on = reactive(False)
    sys_proxy_addr = reactive("")
    up_speed = reactive(0)
    down_speed = reactive(0)
    toast_msg = reactive("Ready")
    core_running = reactive(False)

    def __init__(self, api_url: str = "http://127.0.0.1:9090", secret: str = "", force_mock: bool = False):
        super().__init__()
        self.api_url = api_url
        self.secret = secret
        self.force_mock = force_mock
        self.api = ClashAPI(api_url, secret)
        self.mock = MockClash()
        self.sub_mgr = SubscriptionManager()
        self.core_mgr = CoreManager()
        self.use_mock = force_mock

        self.core_version = "Unknown"
        self.mixed_port = 7890
        self.test_url = "http://www.gstatic.com/generate_204"

        # Proxies data
        self.proxies: Dict[str, dict] = {}
        self.groups: List[str] = []
        self.selected_group = ""
        self.delays: Dict[str, int] = {}
        self.testing_nodes = set()

        # Connections data
        self.conns_data: List[dict] = []
        self.last_up_total = 0
        self.last_down_total = 0

    def compose(self) -> ComposeResult:
        with Horizontal(id="header_bar"):
            yield Label("⚡ CLASH TUI", id="logo")
            with Horizontal(id="status_badges"):
                yield Label("", id="lbl_status")
                yield Label("", id="lbl_mode")
                yield Label("", id="lbl_proxy")
            yield Label("", id="traffic_box")

        with TabbedContent(initial="tab_proxies"):
            with TabPane(" Proxies [1] ", id="tab_proxies"):
                with Horizontal(id="proxies_container"):
                    with Vertical(id="groups_pane"):
                        yield Label("── Proxy Groups ──", classes="card-title")
                        yield DataTable(id="groups_table", cursor_type="row")
                    with Vertical(id="nodes_pane"):
                        yield Label("── Group Nodes ──", id="lbl_nodes_title")
                        yield DataTable(id="nodes_table", cursor_type="row")

            with TabPane(" Connections [2] ", id="tab_conns"):
                yield Label("── Active Network Connections (Press [d] to close, [D] to close all) ──", classes="card-title")
                yield DataTable(id="connections_table", cursor_type="row")

            with TabPane(" Subscriptions [4] ", id="tab_subs"):
                yield Label("── Subscription Profiles ([a] Add, [u] Update, [U] Update All, [Enter] Apply, [x] Delete) ──", classes="card-title")
                yield DataTable(id="subs_table", cursor_type="row")

            with TabPane(" Overview & Settings [3] ", id="tab_settings"):
                with Vertical(id="settings_container"):
                    with Vertical(classes="card"):
                        yield Label("Core Daemon & Connection Info", classes="card-title")
                        yield Static(id="settings_info")
                    with Vertical(classes="card"):
                        yield Label("Keybindings Cheatsheet", classes="card-title")
                        yield Static(
                            " [1, 2, 3, 4]  Switch Tabs (Proxies / Conns / Settings / Subscriptions)\n"
                            " [m]          Cycle Mode (Rule -> Global -> Direct)\n"
                            " [p]          Toggle Windows System Proxy (127.0.0.1:port)\n"
                            " [Enter]      Switch selected node (Proxies) / Apply subscription (Subs)\n"
                            " [t]          Ping highlighted node (ms)\n"
                            " [T]          Ping all nodes in current group\n"
                            " [a]          Add new subscription (in Subscriptions tab)\n"
                            " [u] / [U]    Update selected / update all subscriptions\n"
                            " [x]          Delete selected subscription\n"
                            " [k] / [K]    Stop Core Daemon / Restart Core Daemon\n"
                            " [d] / [D]    Terminate selected connection / terminate all (in Conns tab)\n"
                            " [r]          Force refresh data\n"
                            " [q]          Quit client (Core continues running in background)"
                        )

        yield Label(self.toast_msg, id="toast_bar")
        yield Footer()

    async def on_mount(self) -> None:
        enabled, server = SysProxyManager.get_status()
        self.sys_proxy_on = enabled
        self.sys_proxy_addr = server

        groups_tbl = self.query_one("#groups_table", DataTable)
        groups_tbl.add_columns("Group", "Active Node")

        nodes_tbl = self.query_one("#nodes_table", DataTable)
        nodes_tbl.add_columns("", "Node Name", "Type", "Latency")

        conns_tbl = self.query_one("#connections_table", DataTable)
        conns_tbl.add_columns("Target / Host", "Rule", "Proxy Chain", "Upload", "Download", "Process")

        subs_tbl = self.query_one("#subs_table", DataTable)
        subs_tbl.add_columns("", "Subscription Name", "Status", "Nodes", "Last Updated", "URL")

        # Sync active subscription to data/config.yaml before start
        active_sub = self.sub_mgr.get_active()
        if active_sub:
            cached_profile = os.path.join(self.sub_mgr.profiles_dir, f"{active_sub['id']}.yaml")
            if os.path.exists(cached_profile):
                self.core_mgr.activate_profile(cached_profile)

        # Check / Launch background core
        if not self.force_mock:
            running = await self.core_mgr.is_running(self.api_url, self.secret)
            if not running and self.core_mgr.is_installed():
                self.toast_msg = "Starting Mihomo daemon in background..."
                ok, msg = self.core_mgr.start_core()
                self.toast_msg = msg
                for _ in range(12):
                    await asyncio.sleep(0.3)
                    if await self.core_mgr.is_running(self.api_url, self.secret):
                        break

        await self.refresh_all_data()
        self.render_subs_table()
        self.set_interval(1.0, self.poll_traffic_and_conns)
        self.set_interval(3.0, self.poll_proxies_and_configs)

    def watch_toast_msg(self, msg: str) -> None:
        try:
            self.query_one("#toast_bar", Label).update(f" Status: {msg}")
        except Exception:
            pass

    def watch_is_online(self, val: bool) -> None:
        self.update_header()

    def watch_mode(self, val: str) -> None:
        self.update_header()

    def watch_sys_proxy_on(self, val: bool) -> None:
        self.update_header()

    def watch_up_speed(self, val: int) -> None:
        self.update_traffic_widget()

    def watch_down_speed(self, val: int) -> None:
        self.update_traffic_widget()

    def update_header(self) -> None:
        try:
            status_lbl = self.query_one("#lbl_status", Label)
            if self.is_online:
                status_lbl.update(Text("  ● ONLINE  ", style="bold green"))
            else:
                tag = " (MOCK DEMO) " if self.use_mock else "  ○ OFFLINE  "
                status_lbl.update(Text(tag, style="bold yellow" if self.use_mock else "bold red"))

            mode_color = "cyan" if self.mode.lower() == "rule" else ("magenta" if self.mode.lower() == "global" else "yellow")
            self.query_one("#lbl_mode", Label).update(
                Text(f" Mode: [{self.mode}] ", style=f"bold {mode_color}")
            )

            p_color = "green" if self.sys_proxy_on else "dim white"
            p_text = f" SysProxy: [{'ON' if self.sys_proxy_on else 'OFF'}] "
            self.query_one("#lbl_proxy", Label).update(Text(p_text, style=f"bold {p_color}"))
        except Exception:
            pass

    def update_traffic_widget(self) -> None:
        try:
            txt = Text.assemble(
                ("▲ ", "cyan"),
                (f"{format_speed(self.up_speed)}  ", "white"),
                ("▼ ", "green"),
                (f"{format_speed(self.down_speed)}", "white"),
            )
            self.query_one("#traffic_box", Label).update(txt)
        except Exception:
            pass

    # ==========================================
    # Data Polling & Synchronization
    # ==========================================
    async def refresh_all_data(self) -> None:
        try:
            if not self.force_mock:
                ver = await self.api.get_version()
                self.core_version = ver.get("version", "Mihomo/Clash")
                cfg = await self.api.get_configs()
                self.mode = cfg.get("mode", "Rule")
                self.mixed_port = cfg.get("mixed-port") or cfg.get("port") or 7890
                self.is_online = True
                self.use_mock = False
                self.core_running = True
            else:
                self.use_mock = True
        except Exception:
            self.use_mock = True
            self.is_online = False
            self.core_running = False
            self.core_version = "Simulated Core (Demo Mode)"
            self.toast_msg = "Clash not detected at 9090. Running in Mock/Demo mode."

        await self.poll_proxies_and_configs()
        await self.poll_traffic_and_conns()
        self.render_subs_table()
        self.update_settings_panel()

    async def poll_proxies_and_configs(self) -> None:
        try:
            if not self.use_mock:
                proxies = await self.api.get_proxies()
                cfg = await self.api.get_configs()
                self.mode = cfg.get("mode", self.mode)
                self.is_online = True
                self.core_running = True
            else:
                proxies = self.mock.get_proxies()

            self.proxies = proxies
            groups = [
                name for name, item in proxies.items()
                if item.get("all") and len(item.get("all")) > 0
            ]
            groups.sort(key=lambda x: (0 if x == "GLOBAL" else (1 if "proxy" in x.lower() else 2), x))
            self.groups = groups

            for name, item in proxies.items():
                hist = item.get("history") or []
                if hist and hist[-1].get("delay"):
                    self.delays[name] = hist[-1]["delay"]

            self.render_groups_table()
            self.render_nodes_table()
        except Exception as e:
            if not self.use_mock:
                self.is_online = False
                self.toast_msg = f"API Error: {e}"

    async def poll_traffic_and_conns(self) -> None:
        try:
            if not self.use_mock:
                conns_resp = await self.api.get_connections()
            else:
                self.mock.total_upload += random.randint(1024 * 5, 1024 * 50)
                self.mock.total_download += random.randint(1024 * 50, 1024 * 350)
                conns_resp = self.mock.get_connections()

            up_tot = conns_resp.get("uploadTotal", 0)
            down_tot = conns_resp.get("downloadTotal", 0)

            if self.last_up_total > 0:
                self.up_speed = max(0, up_tot - self.last_up_total)
                self.down_speed = max(0, down_tot - self.last_down_total)
            self.last_up_total = up_tot
            self.last_down_total = down_tot

            self.conns_data = conns_resp.get("connections", [])
            self.render_conns_table()
        except Exception:
            pass

    # ==========================================
    # Rendering Data Tables
    # ==========================================
    def render_groups_table(self) -> None:
        try:
            table = self.query_one("#groups_table", DataTable)
            curr_row = table.cursor_row
            table.clear()
            for g in self.groups:
                now_node = self.proxies.get(g, {}).get("now", "-")
                table.add_row(g, Text(str(now_node), style="cyan"), key=g)

            if self.selected_group not in self.groups and self.groups:
                self.selected_group = self.groups[0]

            if curr_row is not None and curr_row < len(self.groups):
                table.move_cursor(row=curr_row)
        except Exception:
            pass

    def render_nodes_table(self) -> None:
        try:
            if (not self.selected_group or self.selected_group not in self.groups) and self.groups:
                self.selected_group = self.groups[0]
            if not self.selected_group:
                return

            self.query_one("#lbl_nodes_title", Label).update(f"── Nodes in [{self.selected_group}] ──")
            table = self.query_one("#nodes_table", DataTable)
            curr_row = table.cursor_row
            table.clear()

            group_info = self.proxies.get(self.selected_group, {})
            current_active = group_info.get("now", "")
            node_names = group_info.get("all", [])

            for name in node_names:
                is_active = (name == current_active)
                active_icon = Text("●", style="bold green" if is_active else "dim")

                ptype = self.proxies.get(name, {}).get("type", "Proxy")
                ptype_text = Text(str(ptype), style="magenta")

                if name in self.testing_nodes:
                    delay_text = Text("testing...", style="cyan")
                elif name in self.delays:
                    d = self.delays[name]
                    color = "green" if d < 150 else ("yellow" if d < 400 else "red")
                    delay_text = Text(f"{d} ms", style=color)
                else:
                    delay_text = Text("-", style="dim")

                node_styled = Text(str(name), style="bold yellow" if is_active else "white")
                table.add_row(active_icon, node_styled, ptype_text, delay_text, key=name)

            if curr_row is not None and curr_row < len(node_names):
                table.move_cursor(row=curr_row)
        except Exception:
            pass

    def render_conns_table(self) -> None:
        try:
            table = self.query_one("#connections_table", DataTable)
            curr_row = table.cursor_row
            table.clear()
            for c in self.conns_data:
                meta = c.get("metadata", {})
                host = meta.get("host") or meta.get("destinationIP") or "unknown"
                rule = c.get("rule", "Match")
                chains = " -> ".join(c.get("chains", ["DIRECT"]))
                up = format_bytes(c.get("upload", 0))
                down = format_bytes(c.get("download", 0))
                proc = os.path.basename(meta.get("processPath") or "-")
                table.add_row(host, rule, chains, up, down, proc, key=c.get("id"))

            if curr_row is not None and curr_row < len(self.conns_data):
                table.move_cursor(row=curr_row)
        except Exception:
            pass

    def render_subs_table(self) -> None:
        try:
            table = self.query_one("#subs_table", DataTable)
            curr_row = table.cursor_row
            table.clear()
            for s in self.sub_mgr.subscriptions:
                is_active = s.get("active", False)
                active_icon = Text("●", style="bold green" if is_active else "dim")
                status_text = Text("ACTIVE", style="bold green") if is_active else Text("STANDBY", style="dim")
                name_text = Text(s.get("name", "Unnamed"), style="bold yellow" if is_active else "white")
                nodes_text = Text(f"{s.get('node_count', 0)} nodes", style="cyan")
                updated_text = Text(s.get("last_updated", "-"), style="dim")
                url_text = Text(s.get("url", ""), style="dim white")

                table.add_row(active_icon, name_text, status_text, nodes_text, updated_text, url_text, key=s.get("id"))

            if curr_row is not None and curr_row < len(self.sub_mgr.subscriptions):
                table.move_cursor(row=curr_row)
        except Exception:
            pass

    def update_settings_panel(self) -> None:
        try:
            active_sub = self.sub_mgr.get_active()
            sub_name = active_sub.get("name", "None") if active_sub else "None"
            core_state = "Running in Background (Detached)" if self.core_running else "Stopped / Offline"
            txt = (
                f"Core Daemon:         {core_state}\n"
                f"Core Binary:         {self.core_mgr.bin_path}\n"
                f"Core Version:        {self.core_version}\n"
                f"External API:        {self.api_url}\n"
                f"Mixed Proxy Port:    {self.mixed_port}\n"
                f"Active Subscription: {sub_name}\n"
                f"Windows SysProxy:    {'Enabled (' + self.sys_proxy_addr + ')' if self.sys_proxy_on else 'Disabled'}\n"
                f"Delay Test URL:      {self.test_url}\n"
                f"Daemon Lifecycle:    Daemon persists when TUI exits"
            )
            self.query_one("#settings_info", Static).update(txt)
        except Exception:
            pass

    # ==========================================
    # Event Handlers & Key Actions
    # ==========================================
    def on_data_table_row_selected(self, event: DataTable.RowSelected) -> None:
        if event.data_table.id == "groups_table":
            if event.row_key and event.row_key.value:
                self.selected_group = event.row_key.value
                self.render_nodes_table()
        elif event.data_table.id == "nodes_table":
            if event.row_key and event.row_key.value and self.selected_group:
                node_name = event.row_key.value
                self.action_select_node(self.selected_group, node_name)
        elif event.data_table.id == "subs_table":
            if event.row_key and event.row_key.value:
                sub_id = event.row_key.value
                self.apply_subscription(sub_id)

    def on_data_table_row_highlighted(self, event: DataTable.RowHighlighted) -> None:
        if event.data_table.id == "groups_table":
            if event.row_key and event.row_key.value:
                self.selected_group = event.row_key.value
                self.render_nodes_table()

    def action_select_node(self, group: str, node: str) -> None:
        async def do_select():
            self.toast_msg = f"Selecting {group} -> {node}..."
            try:
                if not self.use_mock:
                    await self.api.select_proxy(group, node)
                else:
                    self.mock.active_now[group] = node
                self.toast_msg = f"Switched {group} to {node}"
                await self.poll_proxies_and_configs()
            except Exception as e:
                self.toast_msg = f"Select error: {e}"

        asyncio.create_task(do_select())

    def apply_subscription(self, sub_id: str) -> None:
        sub = self.sub_mgr.set_active(sub_id)
        if sub:
            self.render_subs_table()
            self.update_settings_panel()
            self.toast_msg = f"Applying subscription: {sub['name']}..."

            async def reload_core():
                cached_profile = os.path.join(self.sub_mgr.profiles_dir, f"{sub_id}.yaml")
                if os.path.exists(cached_profile):
                    # 1. Activate profile into data/config.yaml
                    active_config = self.core_mgr.activate_profile(cached_profile)
                    # 2. Tell Mihomo to reload
                    try:
                        await self.api.reload_config(active_config)
                        self.toast_msg = f"Active profile switched to: {sub['name']}"
                    except Exception as e:
                        # Fallback: restart core with new profile
                        self.core_mgr.stop_core()
                        await asyncio.sleep(0.5)
                        self.core_mgr.start_core()
                        for _ in range(12):
                            await asyncio.sleep(0.3)
                            if await self.core_mgr.is_running(self.api_url, self.secret):
                                break
                        self.toast_msg = f"Core restarted with: {sub['name']}"

                    await self.refresh_all_data()

            asyncio.create_task(reload_core())

    def action_add_sub(self) -> None:
        def on_dialog_closed(res: Optional[Tuple[str, str]]) -> None:
            if res:
                name, url = res
                sub = self.sub_mgr.add(name, url)
                self.render_subs_table()
                self.toast_msg = f"Added subscription: {name}"
                self.run_update_sub(sub["id"])

        self.push_screen(AddSubscriptionModal(), on_dialog_closed)

    def action_update_sub(self) -> None:
        try:
            table = self.query_one("#subs_table", DataTable)
            if table.cursor_row is not None and table.cursor_row < len(self.sub_mgr.subscriptions):
                sub_id = self.sub_mgr.subscriptions[table.cursor_row].get("id")
                if sub_id:
                    self.run_update_sub(sub_id)
        except Exception:
            pass

    def run_update_sub(self, sub_id: str) -> None:
        async def do_update():
            self.toast_msg = f"Updating subscription {sub_id}..."
            ok, msg, count = await self.sub_mgr.update_subscription(sub_id)
            self.render_subs_table()
            self.toast_msg = msg
            # If updated subscription is the active one, auto re-apply it
            active_sub = self.sub_mgr.get_active()
            if active_sub and active_sub["id"] == sub_id and ok:
                self.apply_subscription(sub_id)
            else:
                await self.poll_proxies_and_configs()

        asyncio.create_task(do_update())

    def action_update_all_subs(self) -> None:
        async def do_update_all():
            self.toast_msg = f"Updating all {len(self.sub_mgr.subscriptions)} subscriptions..."
            for s in self.sub_mgr.subscriptions:
                await self.sub_mgr.update_subscription(s["id"])
            self.render_subs_table()
            self.toast_msg = "All subscriptions updated"
            active_sub = self.sub_mgr.get_active()
            if active_sub:
                self.apply_subscription(active_sub["id"])
            else:
                await self.poll_proxies_and_configs()

        asyncio.create_task(do_update_all())

    def action_delete_sub(self) -> None:
        try:
            table = self.query_one("#subs_table", DataTable)
            if table.cursor_row is not None and table.cursor_row < len(self.sub_mgr.subscriptions):
                sub = self.sub_mgr.subscriptions[table.cursor_row]
                self.sub_mgr.delete(sub["id"])
                self.render_subs_table()
                self.toast_msg = f"Deleted subscription: {sub['name']}"
        except Exception:
            pass

    def action_cycle_mode(self) -> None:
        next_mode = "Global" if self.mode.lower() == "rule" else ("Direct" if self.mode.lower() == "global" else "Rule")

        async def do_mode():
            self.toast_msg = f"Switching mode to {next_mode}..."
            try:
                if not self.use_mock:
                    await self.api.set_mode(next_mode)
                else:
                    self.mock.mode = next_mode
                self.mode = next_mode
                self.toast_msg = f"Mode set to {next_mode}"
            except Exception as e:
                self.toast_msg = f"Mode error: {e}"

        asyncio.create_task(do_mode())

    def action_toggle_sys_proxy(self) -> None:
        next_state = not self.sys_proxy_on
        server_addr = f"127.0.0.1:{self.mixed_port}"
        ok = SysProxyManager.set_proxy(next_state, server_addr)
        if ok:
            self.sys_proxy_on = next_state
            self.sys_proxy_addr = server_addr if next_state else ""
            self.toast_msg = f"System proxy {'ENABLED' if next_state else 'DISABLED'} ({server_addr})"
        else:
            self.toast_msg = "Failed to toggle Windows system proxy."
        self.update_settings_panel()

    def action_test_latency(self) -> None:
        try:
            nodes_tbl = self.query_one("#nodes_table", DataTable)
            if nodes_tbl.cursor_row is not None and self.selected_group:
                group_nodes = self.proxies.get(self.selected_group, {}).get("all", [])
                if nodes_tbl.cursor_row < len(group_nodes):
                    node = group_nodes[nodes_tbl.cursor_row]
                    self.run_ping_node(node)
        except Exception:
            pass

    def action_test_group_latency(self) -> None:
        if not self.selected_group:
            return
        group_nodes = self.proxies.get(self.selected_group, {}).get("all", [])
        self.toast_msg = f"Testing latency for all {len(group_nodes)} nodes..."
        for node in group_nodes:
            self.run_ping_node(node)

    def run_ping_node(self, node: str) -> None:
        self.testing_nodes.add(node)
        self.render_nodes_table()

        async def do_ping():
            try:
                if not self.use_mock:
                    delay = await self.api.test_delay(node, self.test_url)
                else:
                    await asyncio.sleep(random.uniform(0.1, 0.4))
                    delay = random.randint(60, 320)
                self.delays[node] = delay
                self.toast_msg = f"{node}: {delay} ms"
            except Exception as e:
                self.toast_msg = f"{node} test failed: {e}"
            finally:
                self.testing_nodes.discard(node)
                self.render_nodes_table()

        asyncio.create_task(do_ping())

    def action_stop_core(self) -> None:
        ok, msg = self.core_mgr.stop_core()
        self.core_running = False
        self.toast_msg = msg
        self.update_settings_panel()
        asyncio.create_task(self.refresh_all_data())

    def action_restart_core(self) -> None:
        self.core_mgr.stop_core()

        async def do_restart():
            self.toast_msg = "Restarting Mihomo daemon..."
            await asyncio.sleep(0.5)
            active_sub = self.sub_mgr.get_active()
            if active_sub:
                sub_profile = os.path.join(self.sub_mgr.profiles_dir, f"{active_sub['id']}.yaml")
                if os.path.exists(sub_profile):
                    self.core_mgr.activate_profile(sub_profile)
            ok, msg = self.core_mgr.start_core()
            self.toast_msg = msg
            for _ in range(12):
                await asyncio.sleep(0.3)
                if await self.core_mgr.is_running(self.api_url, self.secret):
                    break
            await self.refresh_all_data()

        asyncio.create_task(do_restart())

    def action_close_conn(self) -> None:
        try:
            table = self.query_one("#connections_table", DataTable)
            if table.cursor_row is not None and table.cursor_row < len(self.conns_data):
                conn_id = self.conns_data[table.cursor_row].get("id")

                async def do_close():
                    if not self.use_mock and conn_id:
                        await self.api.close_connection(conn_id)
                    self.toast_msg = "Connection terminated"
                    await self.poll_traffic_and_conns()

                asyncio.create_task(do_close())
        except Exception:
            pass

    def action_close_all_conns(self) -> None:
        async def do_close_all():
            if not self.use_mock:
                await self.api.close_all_connections()
            self.toast_msg = "All connections closed"
            await self.poll_traffic_and_conns()

        asyncio.create_task(do_close_all())

    def action_refresh_data(self) -> None:
        asyncio.create_task(self.refresh_all_data())

    def action_next_tab(self) -> None:
        tabs = self.query_one(TabbedContent)
        tab_order = ["tab_proxies", "tab_conns", "tab_subs", "tab_settings"]
        try:
            idx = tab_order.index(tabs.active)
            next_idx = (idx + 1) % len(tab_order)
            tabs.active = tab_order[next_idx]
        except Exception:
            tabs.active = "tab_proxies"

    def action_tab_proxies(self) -> None:
        self.query_one(TabbedContent).active = "tab_proxies"

    def action_tab_conns(self) -> None:
        self.query_one(TabbedContent).active = "tab_conns"

    def action_tab_subs(self) -> None:
        self.query_one(TabbedContent).active = "tab_subs"

    def action_tab_settings(self) -> None:
        self.query_one(TabbedContent).active = "tab_settings"


def main():
    parser = argparse.ArgumentParser(description="Clash TUI - Lightweight Terminal Client")
    parser.add_argument("-u", "--url", default="http://127.0.0.1:9090", help="Clash REST API URL (default: http://127.0.0.1:9090)")
    parser.add_argument("-s", "--secret", default="", help="Clash external controller secret")
    parser.add_argument("--mock", action="store_true", help="Run with simulated mock data for demonstration")
    args = parser.parse_args()

    app = ClashTUIApp(api_url=args.url, secret=args.secret, force_mock=args.mock)
    app.run()


if __name__ == "__main__":
    main()
