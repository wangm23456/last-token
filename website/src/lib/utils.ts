import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export const RELEASES_URL = 'https://github.com/wangm23456/last-token/releases'
export const REPO_URL = 'https://github.com/wangm23456/last-token'
