import type { Localized } from '@/shared/lib/i18n';

const ru = {
  categories: {
    documentsMedia: 'Документы и медиа',
    devTools: 'Инструменты разработки',
    codeProjects: 'Проекты кода',
    sdks: 'SDK',
    aiModels: 'Модели ИИ',
    packageCaches: 'Кэши пакетов',
    system: 'Системное',
    other: 'Прочее',
  },
  candidates: {
    userTemp: { title: 'Временные файлы старше 7 дней', hint: '%TEMP%, кроме сессий Claude Code' },
    windowsTemp: {
      title: 'Временные файлы Windows старше 7 дней',
      hint: 'C:\\Windows\\Temp, нужны права администратора',
    },
    pnpmStore: {
      title: 'Хранилище pnpm',
      hint: 'pnpm store prune удалит только пакеты, которые не нужны ни одному проекту',
    },
    npmCache: { title: 'Кэш npm', hint: 'npm cache verify уберёт повреждённое и лишнее' },
    dockerDangling: { title: 'Висячие образы Docker', hint: 'Образы без имени, их не использует ни один контейнер' },
    dockerBuildCache: { title: 'Кэш сборки Docker', hint: 'docker builder prune' },
    nextCache: { title: 'Кэш Next.js', hint: '.next/cache, пересоберётся при следующем запуске' },
    turbo: { title: 'Кэш Turborepo', hint: '.turbo, пересоберётся при следующем запуске' },
    shaderCache: { title: 'Кэш шейдеров', hint: 'Игры и видеодрайвер пересоберут его сами' },
    staleNodeModules: {
      title: 'node_modules проекта без изменений > 30 дней',
      hint: 'pnpm install вернёт их за минуту',
    },
    dockerUnusedImages: {
      title: 'Неиспользуемые образы Docker',
      hint: 'docker image prune -a: пригодятся снова — скачаются заново',
    },
    vhdx: { title: 'Диск WSL или Docker', hint: 'Сжатие вернёт пустое место внутри образа диска' },
  },
} as const;

export const DISK_TEXTS: Localized<typeof ru> = {
  ru,
  en: {
    categories: {
      documentsMedia: 'Documents and media',
      devTools: 'Dev tools',
      codeProjects: 'Code projects',
      sdks: 'SDKs',
      aiModels: 'AI models',
      packageCaches: 'Package caches',
      system: 'System',
      other: 'Other',
    },
    candidates: {
      userTemp: { title: 'Temporary files older than 7 days', hint: '%TEMP%, Claude Code sessions excluded' },
      windowsTemp: {
        title: 'Windows temporary files older than 7 days',
        hint: 'C:\\Windows\\Temp, needs admin rights',
      },
      pnpmStore: { title: 'pnpm store', hint: 'pnpm store prune removes only packages no project needs' },
      npmCache: { title: 'npm cache', hint: 'npm cache verify removes the broken and unneeded' },
      dockerDangling: { title: 'Dangling Docker images', hint: 'Images without a name that no container uses' },
      dockerBuildCache: { title: 'Docker build cache', hint: 'docker builder prune' },
      nextCache: { title: 'Next.js cache', hint: '.next/cache, rebuilt on the next run' },
      turbo: { title: 'Turborepo cache', hint: '.turbo, rebuilt on the next run' },
      shaderCache: { title: 'Shader cache', hint: 'Games and the GPU driver rebuild it themselves' },
      staleNodeModules: {
        title: 'node_modules of a project unchanged > 30 days',
        hint: 'pnpm install brings them back in a minute',
      },
      dockerUnusedImages: {
        title: 'Unused Docker images',
        hint: 'docker image prune -a: needed again — downloaded again',
      },
      vhdx: { title: 'WSL or Docker disk', hint: 'Compacting returns the empty space inside the disk image' },
    },
  },
};
