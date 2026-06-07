import Cookies from "js-cookie";
import {
  createContext,
  type JSX,
  useContext,
  useEffect,
  useState,
} from "react";
import useWebSocket, { ReadyState } from "react-use-websocket";
import type { SendJsonMessage } from "react-use-websocket/dist/lib/types";
import {
  processMessage,
  type SendCommand,
  type ToClientMessage,
} from "../core/messages";
import ErrorScreen from "./ErrorScreen";
import LoadingScreen from "./LoadingScreen";
import { usePushNotification } from "./NotificationProvider";
import { useDispatch } from "./StateProvider";

const WsContext = createContext<SendJsonMessage | null>(null);

declare global {
  interface Window {
    SERVER_URL: string;
  }
}

export const WsProvider = (props: { children: JSX.Element }) => {
  const pushNotification = usePushNotification();
  const [error, setError] = useState<string | null>(null);
  //    const [error, setError] = useState<string | null>(null);
  const dispatch = useDispatch();
  const { sendJsonMessage, lastJsonMessage, readyState } = useWebSocket(
    window.SERVER_URL,
    {
      share: false,
      shouldReconnect: () => false,
    },
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: runs only on connection-state change
  useEffect(() => {
    console.log("Connection state changed", readyState);
    if (readyState === ReadyState.CLOSED) {
      setError("Connection lost");
    }
    if (readyState === ReadyState.OPEN) {
      const authToken = Cookies.get("authToken");
      sendJsonMessage({
        token: authToken,
      });
      sendJsonMessage({
        type: "QueryDir",
        path: "",
      });
      sendJsonMessage({
        type: "QuerySettings",
      });
    }
  }, [readyState]);

  useEffect(() => {
    if (!lastJsonMessage) {
      return;
    }
    console.log("Got a new message: ", lastJsonMessage);
    const message = lastJsonMessage as ToClientMessage;
    processMessage(message, dispatch, pushNotification);
  }, [lastJsonMessage, dispatch, pushNotification]);

  if (error !== null) {
    return <ErrorScreen title="Error" message={error} />;
  }

  if (readyState !== ReadyState.OPEN) {
    return <LoadingScreen />;
  }

  return (
    <WsContext.Provider value={sendJsonMessage}>
      {props.children}
    </WsContext.Provider>
  );
};

export function useSendCommand(): SendCommand {
  const send = useContext(WsContext);
  if (!send) {
    throw new Error("useSendCommand must be used within a WsProvider");
  }
  return send;
}
