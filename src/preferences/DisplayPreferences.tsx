import {
  createContext,
  useContext,
  useLayoutEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export const supportedFontSizes = ["small", "medium", "large", "extra-large"] as const;

export type FontSize = (typeof supportedFontSizes)[number];

interface DisplayPreferencesValue {
  fontSize: FontSize;
  setFontSize: (fontSize: FontSize) => void;
}

const storageKey = "env-manager.font-size";

const DisplayPreferencesContext = createContext<DisplayPreferencesValue>({
  fontSize: "small",
  setFontSize: () => undefined,
});

function initialFontSize(): FontSize {
  try {
    const stored = window.localStorage.getItem(storageKey);
    return supportedFontSizes.find((fontSize) => fontSize === stored) ?? "small";
  } catch {
    return "small";
  }
}

export function DisplayPreferencesProvider({ children }: { children: ReactNode }) {
  const [fontSize, setFontSize] = useState<FontSize>(initialFontSize);

  useLayoutEffect(() => {
    document.documentElement.dataset.fontSize = fontSize;
    try {
      window.localStorage.setItem(storageKey, fontSize);
    } catch {
      // The display preference remains active for this session when storage is unavailable.
    }
  }, [fontSize]);

  const value = useMemo(() => ({ fontSize, setFontSize }), [fontSize]);
  return (
    <DisplayPreferencesContext.Provider value={value}>
      {children}
    </DisplayPreferencesContext.Provider>
  );
}

export function useDisplayPreferences() {
  return useContext(DisplayPreferencesContext);
}
