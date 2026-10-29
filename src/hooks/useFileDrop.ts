import { useState, useCallback, useRef, useEffect } from "react";

function matchesAccept(file: File, accept?: string): boolean {
  if (!accept) return true;

  const patterns = accept.split(",").map((p) => p.trim().toLowerCase());
  const fileType = file.type.toLowerCase();
  const fileExt = `.${file.name.split(".").pop()?.toLowerCase() ?? ""}`;

  return patterns.some((pattern) => {
    if (pattern.startsWith(".")) {
      return fileExt === pattern;
    }
    if (pattern.endsWith("/*")) {
      return fileType.startsWith(pattern.replace("/*", "/"));
    }
    return fileType === pattern;
  });
}

export function useFileDrop(options: {
  accept?: string;
  onDrop: (files: File[]) => void;
}) {
  const { accept, onDrop } = options;
  const [isDragging, setIsDragging] = useState(false);
  const dragCounterRef = useRef(0);

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounterRef.current += 1;
    if (dragCounterRef.current === 1) {
      setIsDragging(true);
    }
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounterRef.current -= 1;
    if (dragCounterRef.current === 0) {
      setIsDragging(false);
    }
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      dragCounterRef.current = 0;
      setIsDragging(false);

      const files = Array.from(e.dataTransfer.files);
      if (files.length === 0) return;

      const filtered = accept
        ? files.filter((f) => matchesAccept(f, accept))
        : files;
      if (filtered.length > 0) {
        onDrop(filtered);
      }
    },
    [accept, onDrop],
  );

  useEffect(() => {
    return () => {
      dragCounterRef.current = 0;
    };
  }, []);

  return {
    isDragging,
    dropHandlers: {
      onDragEnter: handleDragEnter,
      onDragOver: handleDragOver,
      onDragLeave: handleDragLeave,
      onDrop: handleDrop,
    },
  } as const;
}
