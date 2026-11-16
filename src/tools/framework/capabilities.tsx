import { createContext, useContext } from "react";

interface FileDropContextValue {
  isDragging: boolean;
  droppedFiles: File[] | null;
}

const FileDropContext = createContext<FileDropContextValue>({
  isDragging: false,
  droppedFiles: null,
});

export const FileDropProvider = FileDropContext.Provider;

export function useFileDropCapability(): FileDropContextValue {
  return useContext(FileDropContext);
}
