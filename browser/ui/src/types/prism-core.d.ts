declare module "prismjs/components/prism-core" {
  export const languages: { [key: string]: unknown };
  export function highlight(
    code: string,
    grammar: unknown,
    language?: string,
  ): string;
}
