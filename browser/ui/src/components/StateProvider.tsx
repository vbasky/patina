import type { JSX } from "react";
import {
  type ActionDispatch,
  createContext,
  useContext,
  useReducer,
} from "react";
import {
  INITIAL_STATE,
  type State,
  type StateAction,
  stateReducer,
} from "../core/state";

const StateContext = createContext<State>(INITIAL_STATE);
const DispatchContext = createContext<ActionDispatch<
  [action: StateAction]
> | null>(null);

export const StateProvider = (props: { children: JSX.Element }) => {
  const [tasks, dispatch] = useReducer(stateReducer, INITIAL_STATE);

  return (
    <StateContext.Provider value={tasks}>
      <DispatchContext.Provider value={dispatch}>
        {props.children}
      </DispatchContext.Provider>
    </StateContext.Provider>
  );
};

export function useGlobalState(): State {
  return useContext(StateContext);
}

export function useDispatch(): ActionDispatch<[action: StateAction]> {
  const dispatch = useContext(DispatchContext);
  if (!dispatch) {
    throw new Error("useDispatch must be used within a StateProvider");
  }
  return dispatch;
}
