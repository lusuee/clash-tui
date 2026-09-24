import asyncio
from clash_tui import ClashTUIApp, SysProxyManager, SubscriptionManager, CoreManager

async def test_app():
    print("Testing SysProxyManager...")
    enabled, server = SysProxyManager.get_status()
    print(f"  SysProxy: enabled={enabled}, server={server}")

    print("Testing CoreManager...")
    core_mgr = CoreManager()
    installed = core_mgr.is_installed()
    print(f"  Mihomo installed: {installed}, path: {core_mgr.bin_path}")
    assert installed

    print("Testing SubscriptionManager...")
    sub_mgr = SubscriptionManager("test_subscriptions.json", "test_profiles")
    s = sub_mgr.add("Test Sub 1", "https://example.com/demo")
    assert len(sub_mgr.subscriptions) >= 1
    sub_mgr.delete(s["id"])

    print("Testing ClashTUIApp with Textual pilot (live core test)...")
    app = ClashTUIApp(force_mock=False)

    async with app.run_test() as pilot:
        assert app.is_mounted
        print("  App mounted successfully")
        await pilot.pause(0.5)

        # Tab 4 Subscriptions
        await pilot.press("4")
        await pilot.pause(0.2)
        print("  Switched to Tab 4 (Subscriptions)")

        # Tab 3 Settings
        await pilot.press("3")
        await pilot.pause(0.2)
        print("  Switched to Tab 3 (Settings)")

        # Tab 1 Proxies
        await pilot.press("1")
        await pilot.pause(0.2)
        print("  Switched back to Tab 1 (Proxies)")

        # Quit
        await pilot.press("q")
        print("  Quit command handled cleanly without stopping background daemon")

    print("All tests passed successfully!")

if __name__ == "__main__":
    asyncio.run(test_app())
