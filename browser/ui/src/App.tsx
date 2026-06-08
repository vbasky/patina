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
    const searchParams = new URLSearchParams(window.location.search);
    const token = searchParams.get("k");
    if (token) {
      Cookies.set("authToken", token, { path: "/", sameSite: "strict" });
      // Cookies are unreliable in some webviews (WKWebView, used by the Tauri
      // desktop app, drops SameSite=Strict cookies on a non-secure
      // http://127.0.0.1 origin), so also keep the token in sessionStorage,
      // which survives the same-tab redirect. WsProvider reads either source.
      sessionStorage.setItem("authToken", token);
      window.location.href = window.location.origin;
      return; // navigating away — don't mount the app on this pass
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
