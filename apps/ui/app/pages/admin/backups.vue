<script setup lang="ts">
import type { BackupCheck, Backups } from '~/api/types'

// Резервные копии: ежедневные дампы базы из папки на сервере, сколько их и сколько места, когда
// была последняя, и пробная проверка — копия читается целиком, как при восстановлении
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const data = ref<Backups | null>(null)
const failed = ref(false)
async function load() {
  try {
    data.value = await bridge.admin.backups()
    failed.value = false
  }
  catch {
    failed.value = true
  }
}
onMounted(load)

const checking = ref<string | null>(null)
async function check(name?: string) {
  checking.value = name ?? data.value?.files[0]?.name ?? ''
  try {
    const r: BackupCheck = await bridge.admin.checkBackup(name ?? null)
    if (data.value) data.value.lastCheck = r
    if (r.ok) toast.show({ text: t('check.ok', { name: r.name }), icon: 'ph:check-circle' })
    else toast.error(t('check.bad', { name: r.name }))
  }
  catch {
    toast.error(t('check.failed'))
  }
  finally {
    checking.value = null
  }
}
const latest = computed(() => data.value?.files[0] ?? null)
const fresh = computed(() => (latest.value ? Date.now() - new Date(latest.value.createdAt).getTime() < 36 * 3600 * 1000 : false))
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <div v-if="data?.configured && latest" class="ap-head__side">
        <UiButton variant="primary" icon="ph:shield-check" :loading="checking === latest.name" @click="check()">{{ t('check.action') }}</UiButton>
      </div>
    </header>

    <AdminState :loading="!data && !failed" :error="failed && !data" :rows="4" @retry="load">
      <template v-if="data">
        <UiEmptyState v-if="!data.configured" icon="ph:database" :title="t('off.title')" :text="t('off.text')" />
        <template v-else>
          <div class="ap-stats" style="--n: 4">
            <div class="ap-stat" :class="{ 'is-warn': !fresh }">
              <span class="ap-stat__value">{{ latest ? formatRelative(latest.createdAt, lang) : '—' }}</span>
              <span class="ap-stat__label">{{ t('stats.last') }}</span>
            </div>
            <div class="ap-stat"><span class="ap-stat__value tnum">{{ data.files.length }}</span><span class="ap-stat__label">{{ t('stats.count') }}</span></div>
            <div class="ap-stat"><span class="ap-stat__value tnum">{{ formatBytes(data.totalBytes, lang) }}</span><span class="ap-stat__label">{{ t('stats.size') }}</span></div>
            <div class="ap-stat" :class="{ 'is-bad': data.lastCheck && !data.lastCheck.ok }">
              <span class="ap-stat__value">{{ data.lastCheck ? (data.lastCheck.ok ? t('stats.checkOk') : t('stats.checkBad')) : '—' }}</span>
              <span class="ap-stat__label">{{ data.lastCheck ? t('stats.checked', { when: formatRelative(data.lastCheck.checkedAt, lang) }) : t('stats.never') }}</span>
            </div>
          </div>

          <p v-if="!fresh" class="bk__warn" role="status"><Icon name="ph:warning" />{{ t('stale') }}</p>

          <AdminSection v-if="data.lastCheck" :title="t('check.title')">
            <dl class="ap-kv">
              <div><dt>{{ t('check.file') }}</dt><dd>{{ data.lastCheck.name }}</dd></div>
              <div><dt>{{ t('check.result') }}</dt><dd :class="data.lastCheck.ok ? 'ap-ok' : 'ap-bad'">{{ data.lastCheck.ok ? t('check.readable') : data.lastCheck.error }}</dd></div>
              <div v-if="data.lastCheck.tables != null"><dt>{{ t('check.tables') }}</dt><dd>{{ formatNumber(data.lastCheck.tables, lang) }}</dd></div>
              <div v-if="data.lastCheck.entries != null"><dt>{{ t('check.objects') }}</dt><dd>{{ formatNumber(data.lastCheck.entries, lang) }}</dd></div>
            </dl>
          </AdminSection>

          <AdminSection :title="t('files.title')" :hint="t('files.hint')">
            <AdminState :empty="!data.files.length" empty-icon="ph:database" :empty-title="t('files.empty')">
              <div class="ap-table">
                <div class="ap-tr ap-tr--head bk__row">
                  <span>{{ t('files.name') }}</span>
                  <span class="ap-num">{{ t('files.size') }}</span>
                  <span class="ap-num ap-hide-phone">{{ t('files.when') }}</span>
                  <span />
                </div>
                <div v-for="f in data.files" :key="f.name" class="ap-tr bk__row">
                  <span class="ap-cell"><b class="ap-mono bk__name">{{ f.name }}</b></span>
                  <span class="ap-num tnum">{{ formatBytes(f.sizeBytes, lang) }}</span>
                  <span class="ap-num ap-muted ap-hide-phone">{{ formatDateTime(f.createdAt, lang) }}</span>
                  <span class="ap-end">
                    <UiIconButton icon="ph:shield-check" size="sm" :label="t('check.one', { name: f.name })" :disabled="!!checking" @click="check(f.name)" />
                  </span>
                </div>
              </div>
            </AdminState>
          </AdminSection>
        </template>
      </template>
    </AdminState>
  </div>
</template>

<style scoped>
.bk__row { --cols: minmax(0, 1.6fr) 110px 160px 40px; }
.bk__name { color: var(--text-1); }
.bk__warn { display: flex; align-items: center; gap: 10px; margin: -16px 0 0; padding: 12px 16px; border-radius: var(--r-3); background: color-mix(in oklab, var(--warning) 14%, transparent); font-size: var(--fs-small); font-weight: 600; color: var(--warning); }
@media (max-width: 699px) {
  .bk__row { --cols: minmax(0, 1fr) auto 40px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Резервные копии",
    "off": { "title": "Папка с копиями не подключена", "text": "Укажите на сервере DALYWAVE_BACKUP_DIR — папку, куда контейнер backup кладёт ежедневные дампы базы" },
    "stats": { "last": "последняя копия", "count": "копий хранится", "size": "занимают", "checkOk": "Читается", "checkBad": "Повреждена", "checked": "проверка {when}", "never": "ещё не проверялась" },
    "stale": "Свежей копии больше полутора суток нет — проверьте контейнер backup на сервере",
    "check": {
      "action": "Проверить свежую",
      "one": "Проверить {name}",
      "title": "Последняя проверка",
      "file": "Копия",
      "result": "Итог",
      "readable": "Читается целиком, дамп завершён",
      "tables": "Таблиц",
      "objects": "Объектов в схеме",
      "ok": "Копия {name} в порядке",
      "bad": "Копия {name} повреждена",
      "failed": "Не получилось проверить копию"
    },
    "files": { "title": "Копии", "hint": "Копия базы каждый день, хранятся 14 дней", "empty": "Копий пока нет", "name": "Файл", "size": "Размер", "when": "Создана" }
  },
  "uk": {
    "title": "Резервні копії",
    "off": { "title": "Теку з копіями не під’єднано", "text": "Укажіть на сервері DALYWAVE_BACKUP_DIR — теку, куди контейнер backup кладе щоденні дампи бази" },
    "stats": { "last": "остання копія", "count": "копій зберігається", "size": "займають", "checkOk": "Читається", "checkBad": "Пошкоджена", "checked": "перевірка {when}", "never": "ще не перевірялася" },
    "stale": "Свіжої копії немає вже понад півтори доби — перевірте контейнер backup на сервері",
    "check": {
      "action": "Перевірити свіжу",
      "one": "Перевірити {name}",
      "title": "Остання перевірка",
      "file": "Копія",
      "result": "Підсумок",
      "readable": "Читається повністю, дамп завершено",
      "tables": "Таблиць",
      "objects": "Об’єктів у схемі",
      "ok": "Копія {name} в порядку",
      "bad": "Копія {name} пошкоджена",
      "failed": "Не вдалося перевірити копію"
    },
    "files": { "title": "Копії", "hint": "Копія бази щодня, зберігаються 14 днів", "empty": "Копій поки немає", "name": "Файл", "size": "Розмір", "when": "Створено" }
  },
  "en": {
    "title": "Backups",
    "off": { "title": "Backup folder is not connected", "text": "Set DALYWAVE_BACKUP_DIR on the server — the folder where the backup container puts daily database dumps" },
    "stats": { "last": "latest backup", "count": "backups kept", "size": "take up", "checkOk": "Readable", "checkBad": "Damaged", "checked": "checked {when}", "never": "not checked yet" },
    "stale": "No fresh backup for over a day and a half — check the backup container on the server",
    "check": {
      "action": "Check the latest",
      "one": "Check {name}",
      "title": "Last check",
      "file": "Backup",
      "result": "Result",
      "readable": "Fully readable, dump is complete",
      "tables": "Tables",
      "objects": "Schema objects",
      "ok": "Backup {name} is fine",
      "bad": "Backup {name} is damaged",
      "failed": "Couldn't check the backup"
    },
    "files": { "title": "Backups", "hint": "One database backup a day, kept for 14 days", "empty": "No backups yet", "name": "File", "size": "Size", "when": "Created" }
  }
}
</i18n>
