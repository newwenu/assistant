import React from "react";
import { HeartFilled, HeartOutlined } from "@ant-design/icons";
import { useToolStore } from "@/stores/toolStore";

interface FavoriteButtonProps {
  toolId: string;
}

export const FavoriteButton: React.FC<FavoriteButtonProps> = ({ toolId }) => {
  const isFavorite = useToolStore((s) => s.favoriteToolIds.includes(toolId));
  const toggleFavorite = useToolStore((s) => s.toggleFavorite);

  return (
    <span
      onClick={(e) => {
        e.stopPropagation();
        toggleFavorite(toolId);
      }}
      style={{ cursor: "pointer", fontSize: 16, lineHeight: 1 }}
    >
      {isFavorite ? (
        <HeartFilled style={{ color: "#ff4d4f" }} />
      ) : (
        <HeartOutlined style={{ color: "var(--color-text-quaternary)" }} />
      )}
    </span>
  );
};
