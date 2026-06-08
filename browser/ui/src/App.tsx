import { useEffect, useState } from "react";
import "./App.css";
import Cookies from "js-cookie";
import GlobalTabs from "./components/GlobalTabs";
import {
  NotificationProvider,
  Notifications,
} from "./components/NotificationProvider";
import { StateProvider } from "./components/StateProvider";
import { WsProvider } from "./components/WsProvider";

function App() {
  const [tokenChecked, setTokenChecked] = useState<boolean>(false);

  useEffect(() => {
    const token = new URLSearchParams(window.location.search).get("k");
    if (token) {
      // Persist for later same-origin navigations. We deliberately do NOT
      // redirect to strip the param: the Tauri desktop webview (WKWebView)
      // doesn't reliably keep a cookie across that redirect on a non-secure
      // http://127.0.0.1 origin, which broke auth. Keeping ?k= in the URL means
      // WsProvider can read the token directly — no cookie/redirect dependency.
      Cookies.set("authToken", token, { path: "/", sameSite: "strict" });
      sessionStorage.setItem("authToken", token);
    }
    setTokenChecked(true);
  }, []);

  return (
    tokenChecked && (
      <NotificationProvider>
        <Notifications />
        <StateProvider>
          <WsProvider>
            <div className="h-screen w-screen flex fixed top-0 left-0">
              <GlobalTabs />
            </div>
          </WsProvider>
        </StateProvider>
      </NotificationProvider>
    )
  );
}

export default App;
