// Dusk's icons are SF Symbols from GlyphStudio. glyphs.ts is generated from glyph-names.json by scripts/sync-glyphs.mjs.
import type { SVGProps } from "react";
import { glyphs } from "./glyphs";

type GlyphName = keyof typeof glyphs;
type IconProps = Omit<SVGProps<SVGSVGElement>, "fill"> & {
  size?: number;
  /** "currentColor" draws the filled variant when one exists, matching the old lucide prop. */
  fill?: string;
};

function glyph(name: GlyphName, filled?: GlyphName) {
  return function Glyph({ size = 24, fill, ...rest }: IconProps) {
    const [width, height, ...paths] = glyphs[fill === "currentColor" && filled ? filled : name];
    // SF Symbols fill their viewport edge to edge; pad the box so they sit like the old 24px line icons.
    const box = Math.max(width, height) * 1.2;
    const viewBox = `${(width - box) / 2} ${(height - box) / 2} ${box} ${box}`;
    return (
      <svg width={size} height={size} viewBox={viewBox} fill="currentColor" aria-hidden="true" focusable="false" {...rest}>
        {paths.map((d, index) => <path key={index} d={d} />)}
      </svg>
    );
  };
}

export const Archive = glyph("Archive");
export const ArrowLeft = glyph("ArrowLeft");
export const Camera = glyph("Camera");
export const Check = glyph("Check");
export const ChevronLeft = glyph("ChevronLeft");
export const ChevronRight = glyph("ChevronRight");
export const Clock3 = glyph("Clock3");
export const Cloud = glyph("Cloud");
export const Database = glyph("Database");
export const Download = glyph("Download");
export const ExternalLink = glyph("ExternalLink");
export const FileCode2 = glyph("FileCode2");
export const FolderOpen = glyph("FolderOpen");
export const FolderPlus = glyph("FolderPlus");
export const Gamepad2 = glyph("Gamepad2");
export const Globe2 = glyph("Globe2");
export const HardDrive = glyph("HardDrive");
export const Heart = glyph("Heart", "HeartFill");
export const Home = glyph("Home");
export const ImagePlus = glyph("ImagePlus");
export const KeyRound = glyph("KeyRound");
export const Library = glyph("Library");
export const LoaderCircle = glyph("LoaderCircle");
export const LockKeyhole = glyph("LockKeyhole");
export const LogIn = glyph("LogIn");
export const Mail = glyph("Mail");
export const Minus = glyph("Minus");
export const Moon = glyph("Moon");
export const PackageCheck = glyph("PackageCheck");
export const PackageOpen = glyph("PackageOpen");
export const Pencil = glyph("Pencil");
export const Play = glyph("Play", "PlayFill");
export const Plus = glyph("Plus");
export const Radio = glyph("Radio");
export const RefreshCw = glyph("RefreshCw");
export const RotateCcw = glyph("RotateCcw");
export const Save = glyph("Save");
export const ScanSearch = glyph("ScanSearch");
export const Search = glyph("Search");
export const Settings = glyph("Settings");
export const ShieldCheck = glyph("ShieldCheck");
export const Square = glyph("Square");
export const Star = glyph("Star", "StarFill");
export const Trash2 = glyph("Trash2");
export const Trophy = glyph("Trophy");
export const Upload = glyph("Upload");
export const UserPlus = glyph("UserPlus");
export const UserRound = glyph("UserRound");
export const X = glyph("X");
export const Zap = glyph("Zap");
