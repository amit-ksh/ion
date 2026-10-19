/// <reference types="vite/client" />
import {
  HeadContent,
  Outlet,
  Scripts,
  createRootRouteWithContext,
} from '@tanstack/react-router'
import type { QueryClient } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import appCss from '@/styles.css?url'

// Set only when an Ion domain has actually been chosen.
const SITE_URL = import.meta.env.VITE_ION_SITE_URL?.replace(/\/$/, '')
const TITLE = 'Ion — a workspace for coding agents, teams and workflows'
const DESCRIPTION =
  'Bring coding harnesses, hierarchical agent teams, connected apps, workflows and document artifacts together in Ion. Explore Windows and Android release details.'

export const Route = createRootRouteWithContext<{
  queryClient: QueryClient
}>()({
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      { title: TITLE },
      { name: 'description', content: DESCRIPTION },
      { property: 'og:title', content: TITLE },
      { property: 'og:description', content: DESCRIPTION },
      { property: 'og:type', content: 'website' },
      ...(SITE_URL ? [
        { property: 'og:url', content: SITE_URL },
        { property: 'og:image', content: `${SITE_URL}/og-icon.png` },
      ] : []),
      { property: 'og:site_name', content: 'Ion' },
      { name: 'twitter:card', content: 'summary' },
      {
        name: 'theme-color',
        content: '#ffffff',
      },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: 'icon', type: 'image/svg+xml', href: '/ion-favicon.svg' },
      { rel: 'icon', type: 'image/png', href: '/favicon.png' },
      { rel: 'apple-touch-icon', sizes: '180x180', href: '/apple-touch-icon.png' },
      ...(SITE_URL ? [{ rel: 'canonical', href: SITE_URL }] : []),
    ],
  }),
  component: RootComponent,
})

function RootComponent() {
  return (
    <RootDocument>
      <Outlet />
    </RootDocument>
  )
}

function RootDocument({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}
