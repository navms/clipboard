import { useEffect } from "react";
import { Shell } from "./components/Shell";
import { useClipEvents } from "./hooks/useClipEvents";
import { useKeyboardNav } from "./hooks/useKeyboardNav";
import { useApp } from "./stores/useApp";

export default function App() {
  const init = useApp((s) => s.init);

  useEffect(() => {
    void init();
  }, [init]);

  useKeyboardNav();
  useClipEvents();

  return <Shell />;
}
