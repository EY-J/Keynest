import {
  Component,
  lazy,
  Suspense,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import "./LockScreenBackground.css";

const PixelBlast = lazy(() => import("../../../shared/components/PixelBlast/PixelBlast"));

// Decorative assets must not prevent access to the password/recovery form.
class BackgroundFallback extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}

export default function LockScreenBackground({ paused }: { paused: boolean }) {
  const [motionAllowed, setMotionAllowed] = useState(false);

  useEffect(() => {
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setMotionAllowed(!preference.matches);
    update();
    preference.addEventListener("change", update);
    return () => preference.removeEventListener("change", update);
  }, []);

  if (!motionAllowed || paused) return null;

  return (
    <div className="lock-screen-background" aria-hidden="true">
      <BackgroundFallback>
        <Suspense fallback={null}>
          <PixelBlast
            variant="circle"
            pixelSize={6}
            color="#54f5ae"
            patternScale={3}
            patternDensity={1.2}
            pixelSizeJitter={0.5}
            enableRipples={false}
            liquid={false}
            speed={0.6}
            edgeFade={0.25}
            transparent
          />
        </Suspense>
      </BackgroundFallback>
    </div>
  );
}
