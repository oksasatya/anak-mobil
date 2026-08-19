/**
 * Types for the derived-value helpers. Hand-written so `derive.js` can stay
 * plain JavaScript that Node, Bun, and Metro all import without a build step.
 *
 * Keep in sync with `derive.js`.
 */

export declare function relativeLuminance(hex: string): number;
export declare function contrastRatio(a: string, b: string): number;
export declare function meetsAA(foreground: string, background: string, large?: boolean): boolean;
export declare function composite(tint: string, coverage: number, backdrop: string): string;
export declare function mix(base: string, other: string, weight: number): string;
export declare function withAlpha(hex: string, alpha: number): string;

export interface ParsedType {
  readonly fontWeight: number;
  readonly fontSize: number;
  readonly lineHeight: number;
}

export declare function parseTypeShorthand(value: string): ParsedType;
