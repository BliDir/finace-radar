import { LogOut, RefreshCw } from "lucide-react";

export function UserMenu({
  user,
  email,
  isOpen,
  isAnalyzing,
  isLoading,
  onToggle,
  onAnalyze,
  onLogout,
  t,
}) {
  const initials = (user.name || email || "?")
    .split(/\s|@/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("");

  return (
    <div className="user-menu">
      <button
        type="button"
        className="avatar-button"
        onClick={onToggle}
        aria-expanded={isOpen}
        aria-haspopup="menu"
        aria-label={t.profile}
      >
        {user.picture ? (
          <img src={user.picture} alt={user.name || email} referrerPolicy="no-referrer" />
        ) : (
          <span>{initials}</span>
        )}
      </button>

      {isOpen && (
        <div className="user-dropdown" role="menu">
          <div className="user-dropdown-header">
            <strong>{user.name || email}</strong>
            <span>{email}</span>
          </div>
          <button type="button" onClick={onAnalyze} disabled={isAnalyzing || isLoading} role="menuitem">
            <RefreshCw size={16} /> {isAnalyzing ? t.analyzing : t.analyzeInbox}
          </button>
          <button type="button" onClick={onLogout} disabled={isAnalyzing} role="menuitem">
            <LogOut size={16} /> {t.signOut}
          </button>
        </div>
      )}
    </div>
  );
}
