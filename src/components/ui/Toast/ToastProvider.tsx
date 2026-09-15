import {
  type AnimationEvent,
  type ReactNode,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import "./toast.css";

const TOAST_VISIBLE_MS = 2_200;
const TOAST_EXIT_FALLBACK_MS = 250;

export type ToastType = "success" | "error" | "info";

export type ToastOptions = {
  type: ToastType;
  message: string;
  detail?: string;
};

type ToastPhase = "visible" | "exiting";
type ToastContextValue = {
  showToast(options: ToastOptions): void;
};

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [content, setContent] = useState<ToastOptions | null>(null);
  const [phase, setPhase] = useState<ToastPhase | null>(null);
  const [cycle, setCycle] = useState(0);
  const exitTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const removalTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);

  const clearToastTimers = useCallback(() => {
    if (exitTimer.current !== null) {
      window.clearTimeout(exitTimer.current);
      exitTimer.current = null;
    }
    if (removalTimer.current !== null) {
      window.clearTimeout(removalTimer.current);
      removalTimer.current = null;
    }
  }, []);

  const showToast = useCallback((options: ToastOptions) => {
    clearToastTimers();
    setContent(options);
    setCycle((current) => current + 1);
    setPhase("visible");
    exitTimer.current = window.setTimeout(() => {
      exitTimer.current = null;
      setPhase("exiting");
      removalTimer.current = window.setTimeout(() => {
        removalTimer.current = null;
        setPhase(null);
      }, TOAST_EXIT_FALLBACK_MS);
    }, TOAST_VISIBLE_MS);
  }, [clearToastTimers]);

  useEffect(() => clearToastTimers, [clearToastTimers]);

  function finishExit(event: AnimationEvent<HTMLElement>) {
    if (phase !== "exiting" || event.currentTarget !== event.target) return;
    if (removalTimer.current !== null) {
      window.clearTimeout(removalTimer.current);
      removalTimer.current = null;
    }
    setPhase(null);
  }

  const value = useMemo(() => ({ showToast }), [showToast]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      {phase && content ? (
        <aside
          key={cycle}
          className={`keynest-toast is-${phase} is-${content.type}`}
          role="status"
          aria-live="polite"
          aria-atomic="true"
          onAnimationEnd={finishExit}
        >
          <strong>{content.message}</strong>
          {content.detail ? <span>{content.detail}</span> : null}
        </aside>
      ) : null}
    </ToastContext.Provider>
  );
}

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) throw new Error("useToast must be used inside ToastProvider");
  return context;
}
