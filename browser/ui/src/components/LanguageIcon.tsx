import { SiJavascript, SiPython, SiRust } from "react-icons/si";
import type { Language } from "../core/notebook";

// Brand icon for a notebook language, in its conventional accent color.
export const LanguageIcon: React.FC<{ language: Language; size?: number }> = ({
  language,
  size = 14,
}) => {
  switch (language) {
    case "Rust":
      return <SiRust size={size} className="text-orange-600 dark:text-orange-400" />;
    case "Python":
      return <SiPython size={size} className="text-blue-500" />;
    case "JavaScript":
      return <SiJavascript size={size} className="text-yellow-500" />;
  }
};
