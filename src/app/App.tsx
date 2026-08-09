import "../App.css";
import HomePage from "../pages/HomePage";

export default function App() {
  function openPasswordVault() {
    alert("The Password Vault page will open here.");
  }

  return (
    <HomePage
      onOpenPasswordVault={openPasswordVault}
    />
  );
}