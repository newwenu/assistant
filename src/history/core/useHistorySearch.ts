import { useState, useRef, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Conversation } from "../../chat/chatTypes";

interface UseHistorySearchReturn {
  searchQuery: string;
  searchResults: Conversation[] | null;
  searchFullText: boolean;
  setSearchFullText: React.Dispatch<React.SetStateAction<boolean>>;
  handleSearch: (value: string) => void;
}

export function useHistorySearch(
  conversations: Conversation[],
): UseHistorySearchReturn {
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Conversation[] | null>(
    null,
  );
  const [searchFullText, setSearchFullText] = useState(false);

  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleSearch = useCallback(
    (value: string) => {
      setSearchQuery(value);
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
      if (!value.trim()) {
        setSearchResults(null);
        return;
      }
      searchTimerRef.current = setTimeout(() => {
        void (async () => {
          try {
            if (searchFullText) {
              const result = await invoke("search_conversations", {
                query: value,
              });
              setSearchResults((result as Conversation[]) || []);
            } else {
              const q = value.toLowerCase();
              const filtered = conversations.filter((c) =>
                c.title.toLowerCase().includes(q),
              );
              setSearchResults(filtered);
            }
          } catch (e) {
            console.error("Search failed:", e);
            setSearchResults([]);
          }
        })();
      }, 300);
    },
    [searchFullText, conversations],
  );

  const searchQueryRef = useRef(searchQuery);
  searchQueryRef.current = searchQuery;

  useEffect(() => {
    const q = searchQueryRef.current;
    if (q.trim()) handleSearch(q);
  }, [searchFullText, handleSearch]);

  return {
    searchQuery,
    searchResults,
    searchFullText,
    setSearchFullText,
    handleSearch,
  };
}
