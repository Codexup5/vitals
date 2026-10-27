import type { Plugin } from 'vite';

export type CspMode = 'enforce' | 'report-only';

export interface CspConfig {
  mode: CspMode;
  directives: Record<string, string[]>;
  reportUri?: string;
}

export function readCspConfig(root: string): CspConfig | null;

export function toPolicy(config: CspConfig, delivery: 'header' | 'meta'): string;

export function cspHeader(config: CspConfig): { key: string; value: string };

export interface CspHeader {
  key: string;
  value: string;
}

export function listCspHeaders(vercel: unknown): CspHeader[];

export function fromCspHeader(header: CspHeader): CspConfig | null;

export function cspPlugin(): Plugin;
