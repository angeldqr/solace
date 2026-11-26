import { createContext, type ReactNode, useContext } from "react";

const Context = createContext<string | undefined>(undefined);

/** Lets a row point its controls at its description (aria-describedby). */
export const DescribedBy = ({ value, children }: { value?: string; children: ReactNode }) => (
  <Context.Provider value={value}>{children}</Context.Provider>
);

export const useDescribedBy = () => useContext(Context);
