export interface paths {
    "/api/acquisitions/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["acquisition"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/acquisitions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/acquisitions/{id}/resolve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resolve"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/audit": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["audit_log"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/downloaders": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_downloaders"];
        put?: never;
        post: operations["create_downloader"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/downloaders/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["revoke_downloader"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/jobs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_jobs"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/jobs/{id}/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancel_job"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/jobs/{id}/retry": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["retry_job"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/lanes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_lanes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/lanes/{lane}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_lane"];
        trace?: never;
    };
    "/api/admin/overview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["overview"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/reco": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["reco_overview"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/reco/run": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["run_reco"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/reco/settings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_reco_settings"];
        trace?: never;
    };
    "/api/admin/sessions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_sessions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/sessions/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["revoke_session"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/sources": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["sources"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/users": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_users"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/users/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_user"];
        trace?: never;
    };
    "/api/albums/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["album"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/artists/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["artist"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/app/start": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["app_start"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/app/token": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["app_token"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/logout": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["logout"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/me": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["me"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/oidc/callback": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["oidc_callback"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/oidc/start": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["oidc_start"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/password": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["password_login"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/providers": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["providers"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/catalog/deezer/{kind}/{deezerId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["resolve_deezer"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["ingest"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/fetcher/jobs/claim": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["claim"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/fetcher/jobs/{id}/lease": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["renew_lease"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/fetcher/jobs/{id}/release": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["release"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/fetcher/jobs/{id}/result": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/healthz": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["healthz"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/home": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["home"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/images/{kind}/{id}/{file}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["image"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["overview_get_library"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/albums": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["saved_albums"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/albums/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["save_album"];
        post?: never;
        delete: operations["unsave_album"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/artists": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["followed_artists"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/artists/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["follow"];
        post?: never;
        delete: operations["unfollow"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/blocked": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["blocked"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/blocked/{kind}/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["block"];
        post?: never;
        delete: operations["unblock"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/tracks": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["liked_tracks"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/library/tracks/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["like"];
        post?: never;
        delete: operations["unlike"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/me/history": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["history"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/openapi.json": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["openapi"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_get_playlists"];
        put?: never;
        post: operations["create"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["detail"];
        put?: never;
        post?: never;
        delete: operations["remove"];
        options?: never;
        head?: never;
        patch: operations["update"];
        trace?: never;
    };
    "/api/playlists/{id}/items": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["add_items"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/items/{itemId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["remove_item"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/items/{itemId}/move": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["move_item"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/pin": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["pin"];
        post?: never;
        delete: operations["unpin"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["search"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/search/sound": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["sound_search"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/sync": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["sync"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/lookup": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["lookup_tracks"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["track"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}/similar": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["similar"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}/stream": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["stream"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/updates/{target}/{arch}/{current_version}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["check"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/wave/sessions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["create_session"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/wave/sessions/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["session"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/wave/sessions/{id}/feedback": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["feedback"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/wave/sessions/{id}/next": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["next"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/ws": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["ws"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /**
         * @description Данные задачи `acquire`: что скачать. Тот же набор полей сервер отдаёт домашнему загрузчику
         *     (`POST /api/fetcher/jobs/claim`) вместе с подписанной ссылкой для файла.
         */
        AcquirePayload: {
            /** Format: uuid */
            acquisitionId: string;
            /**
             * Format: int64
             * @description fetcher ищет запись по треку Deezer.
             */
            deezerTrackId: number;
            /** @description Ссылка, вставленная в «Спорных» (SoundCloud, Bandcamp, …): качается она, без поиска. */
            directUrl?: string | null;
            /** @description Отклонённые видео. */
            excludeVideoIds?: string[];
            /** @description Видео, выбранные вручную в «Спорных»: пробуются первыми. */
            preferVideoIds?: string[];
            /** Format: uuid */
            recordingId: string;
            /** Format: uuid */
            trackId: string;
        };
        /**
         * @description Зачем скачивается запись. Хранится в `acquisitions.reason`.
         * @enum {string}
         */
        AcquireReason: "play" | "wave" | "like" | "playlist" | "album" | "subscription" | "discovery" | "import" | "manual" | "upgrade";
        /** @description Скачивание записи: шаг, место в очереди, есть ли кому качать. */
        Acquisition: {
            /**
             * Format: int32
             * @description Сколько раз задачу брали в работу.
             */
            attempts: number;
            audioStatus: components["schemas"]["AudioStatus"];
            /** Format: date-time */
            createdAt: string;
            /**
             * Format: int64
             * @description Загрузчики на связи (обращались в последние две минуты).
             */
            downloadersOnline: number;
            error: string | null;
            /** Format: date-time */
            finishedAt: string | null;
            /** Format: uuid */
            id: string;
            lane: string;
            /**
             * Format: int64
             * @description Сколько скачиваний впереди; 0 — следующее или уже идёт; `null`, если скачивание закончилось.
             */
            queuePosition: number | null;
            reason: components["schemas"]["AcquireReason"];
            /** Format: uuid */
            recordingId: string;
            status: components["schemas"]["AcquisitionStatus"];
            /** Format: uuid */
            trackId: string;
            /** Format: date-time */
            updatedAt: string;
            /** @description В очереди, но качать некому: загрузчиков на связи нет, серверный fetcher выключен. */
            waitingForDownloader: boolean;
        };
        /** @description Попытка скачать запись: кто качал, что ответил fetcher и что решил сервер. */
        AcquisitionAttempt: {
            /** Format: date-time */
            createdAt: string;
            error: string | null;
            errorCode: string | null;
            /** @description `server` или `device:<id>` (домашний загрузчик). */
            executor: string;
            /** Format: uuid */
            id: string;
            outcome: components["schemas"]["AttemptOutcome"];
            /** @description Статус ответа fetcher: `verified`, `unverified`, `disputed`, `not_found`, `error`. */
            status: string;
            /** Format: int32 */
            totalMs: number | null;
            videoId: string | null;
            ytDlpVersion: string | null;
        };
        /** @description Кандидат из поиска fetcher: оценка с разбором и что вышло при попытке скачать. */
        AcquisitionCandidate: {
            album: string | null;
            artists: string[];
            attempt: components["schemas"]["CandidateAttempt"] | null;
            /** @description Разбор оценки по составляющим: длительность, название, артисты, альбом, источник, множители. */
            breakdown: Record<string, never>;
            /** Format: int64 */
            durationMs: number | null;
            /** @description `atv` — официальное аудио, `omv` — клип артиста, `ugc` и `other` — прочее. */
            kind: string | null;
            /** @description Почему отброшен (длительность, название). */
            rejected: string | null;
            /**
             * Format: double
             * @description 0..1.
             */
            score: number | null;
            /** @description Как найден: `album`, `isrc`, `song`, `video`, `manual`. */
            source: string | null;
            title: string;
            url: string | null;
            /** @description id видео YouTube или ссылка, вставленная вручную. */
            videoId: string;
        };
        /**
         * @description Шаг поиска и скачивания одной записи. Хранится в `acquisitions.status`.
         * @enum {string}
         */
        AcquisitionStatus: "queued" | "matching" | "downloading" | "verifying" | "processing" | "ready" | "disputed" | "unavailable" | "failed" | "cancelled";
        AcquisitionUpdate: {
            audioStatus: components["schemas"]["AudioStatus"];
            error: string | null;
            /** Format: uuid */
            id: string;
            /** Format: uuid */
            recordingId: string;
            status: components["schemas"]["AcquisitionStatus"];
            /** Format: uuid */
            trackId: string;
        };
        /** @description Добавить треки: в конец, в начало или после позиции. */
        AddPlaylistItems: {
            /**
             * Format: uuid
             * @description После этой позиции; без неё — в конец (или в начало при `atStart`).
             */
            afterItemId?: string | null;
            atStart?: boolean;
            trackIds: string[];
        };
        AddedPlaylistItems: {
            items: components["schemas"]["PlaylistItem"][];
            /** @description Позиции остальных треков пересчитаны: интерфейсу стоит перечитать плейлист. */
            rebalanced: boolean;
        };
        /** @description Скачивание в разделах «Загрузки» и «Спорные». */
        AdminAcquisition: {
            acquisition: components["schemas"]["Acquisition"];
            /** @description Попытки, новые первыми. */
            attemptsLog: components["schemas"]["AcquisitionAttempt"][];
            /** @description Кандидаты по убыванию оценки, с разбором и попытками. */
            candidates: components["schemas"]["AcquisitionCandidate"][];
            /** @description Выбранный кандидат, скачивание и проверка — как прислал fetcher. */
            chosen: Record<string, never> | null;
            directUrl: string | null;
            /** Format: uuid */
            jobId: string | null;
            jobStatus: components["schemas"]["JobStatus"] | null;
            /** @description Видео, выбранные вручную, и отклонённые — из задачи. */
            preferVideoIds: string[];
            /** Format: uuid */
            requestedBy: string | null;
            track: components["schemas"]["Track"] | null;
            /** @description Исполнитель последней попытки: `server` или `device:<id>`. */
            worker: string | null;
        };
        AdminAcquisitionPage: {
            items: components["schemas"]["AdminAcquisition"][];
            nextCursor: string | null;
        };
        /** @description Раздел «Обзор» админки. */
        AdminOverview: {
            lanes: components["schemas"]["Lane"][];
            last24h: components["schemas"]["DayStats"];
            library: components["schemas"]["LibraryStats"];
            /** @description Записи по состоянию аудио; `none` тоже здесь. */
            recordingsByStatus: components["schemas"]["StatusCount"][];
            storage: components["schemas"]["StorageStats"];
        };
        /** @description Пользователь в разделе «Доступ». */
        AdminUser: {
            avatarUrl: string | null;
            /** Format: date-time */
            createdAt: string;
            disabled: boolean;
            email: string | null;
            emailVerified: boolean;
            /** Format: uuid */
            id: string;
            /** @description Роль admin выдана daly.rest и снимется, когда её снимут там. */
            idpAdmin: boolean;
            /** @description Провайдер входа; `null` у локального владельца, созданного аварийным входом. */
            issuer: string | null;
            /** Format: date-time */
            lastLoginAt: string | null;
            name: string;
            roles: components["schemas"]["Role"][];
        };
        /** @description Страница альбома. */
        AlbumDetail: {
            album: components["schemas"]["AlbumSummary"];
            /** Format: int32 */
            fans: number | null;
            genres: string[];
            label: string | null;
            /**
             * Format: int64
             * @description Сколько треков уже на сервере.
             */
            readyTracks: number;
            /**
             * Format: int64
             * @description Сумма длительностей треков.
             */
            totalDurationMs: number;
            /** @description По порядку: диск, затем позиция. */
            tracks: components["schemas"]["Track"][];
            upc: string | null;
        };
        AlbumRef: {
            /** Format: uuid */
            id: string;
            image: components["schemas"]["ImageSet"] | null;
            title: string;
        };
        /** @description Карточка альбома в списках. */
        AlbumSummary: {
            artist: components["schemas"]["ArtistRef"];
            /** Format: int64 */
            deezerId: number | null;
            /** Format: int32 */
            durationMs: number | null;
            explicit: boolean;
            /** Format: uuid */
            id: string;
            image: components["schemas"]["ImageSet"] | null;
            /** @description 3–5 основных цветов обложки (`#rrggbb`, самый заметный первым); `null`, пока обложка не скопирована. */
            palette: string[] | null;
            recordType: components["schemas"]["RecordType"];
            /** Format: date */
            releaseDate: string | null;
            saved: boolean;
            title: string;
            /** Format: int32 */
            tracksCount: number | null;
        };
        AnalyzeFullPayload: {
            /** Format: uuid */
            recordingId: string;
        };
        /** @description Ссылка на превью у Deezer подписана и живёт недолго: задача берёт свежую в момент анализа. */
        AnalyzePreviewPayload: {
            /** Format: uuid */
            trackId: string;
        };
        AppToken: {
            device: components["schemas"]["Device"];
            /** @description `dwd_…`: передаётся как `Authorization: Bearer`. Показывается один раз. */
            token: string;
            user: components["schemas"]["User"];
        };
        AppTokenRequest: {
            appVersion?: string | null;
            /** @description Одноразовый код из адреса возврата, живёт 60 секунд. */
            code: string;
            codeVerifier: string;
        };
        /** @description Страница артиста. */
        ArtistDetail: {
            artist: components["schemas"]["ArtistSummary"];
            follow: components["schemas"]["FollowState"] | null;
            related: components["schemas"]["ArtistSummary"][];
            releases: components["schemas"]["Releases"];
            topTracks: components["schemas"]["Track"][];
            /**
             * Format: int64
             * @description Сколько треков дискографии уже на сервере («скачано 48 из 132»).
             */
            tracksOnServer: number;
            /** Format: int64 */
            tracksTotal: number;
        };
        ArtistRef: {
            /** Format: uuid */
            id: string;
            name: string;
        };
        /** @enum {string} */
        ArtistRole: "main" | "featured";
        /** @description Карточка артиста в списках. */
        ArtistSummary: {
            /** Format: int32 */
            albumsCount: number | null;
            /** Format: int64 */
            deezerId: number | null;
            /** Format: int32 */
            fans: number | null;
            followed: boolean;
            /** Format: uuid */
            id: string;
            image: components["schemas"]["ImageSet"] | null;
            name: string;
        };
        /**
         * @description Решение сервера по одной попытке.
         * @enum {string}
         */
        AttemptOutcome: "ready" | "disputed" | "unavailable" | "retry" | "failed" | "postponed";
        /**
         * @description Кодек аудиофайла. Хранится в `audio_files.codec`.
         * @enum {string}
         */
        AudioCodec: "opus" | "vorbis" | "aac" | "mp3" | "flac" | "alac" | "pcm";
        /**
         * @description Контейнер аудиофайла. Хранится в `audio_files.container`.
         * @enum {string}
         */
        AudioContainer: "ogg" | "webm" | "mp4" | "mp3" | "flac" | "wav";
        /**
         * @description Есть ли аудио записи на сервере. Интерфейс показывает это значками.
         * @enum {string}
         */
        AudioStatus: "none" | "queued" | "acquiring" | "ready" | "disputed" | "unavailable" | "failed";
        AuditActor: {
            /** Format: uuid */
            id: string;
            name: string;
        };
        /** @description Событие журнала: входы, смены ролей, отзывы сессий. */
        AuditEvent: {
            actor: components["schemas"]["AuditActor"] | null;
            /** Format: date-time */
            at: string;
            /** Format: uuid */
            id: string;
            /** @description Вид события, например `auth.sign_in` или `user.roles_changed`. */
            kind: string;
            payload: Record<string, never>;
            /** @description Над чем: id пользователя, сессии или устройства. */
            subject: string | null;
        };
        AuditPage: {
            items: components["schemas"]["AuditEvent"][];
            nextCursor: string | null;
        };
        /** @description Какие способы входа включены на сервере. */
        AuthProviders: {
            oidc: components["schemas"]["OidcProvider"] | null;
            /** @description Включён ли аварийный вход паролем. */
            password: boolean;
        };
        /**
         * @description Как открыта текущая сессия.
         * @enum {string}
         */
        AuthVia: "oidc" | "password" | "device";
        /** @description «Не рекомендовать»: трек или артист. */
        BlockedItem: {
            artist: components["schemas"]["ArtistSummary"] | null;
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["BlockedKind"];
            track: components["schemas"]["Track"] | null;
        };
        /** @enum {string} */
        BlockedKind: "track" | "artist";
        BlockedList: {
            items: components["schemas"]["BlockedItem"][];
        };
        /** @description Чем кончилась попытка скачать кандидата. */
        CandidateAttempt: {
            downloaded: boolean;
            error: string | null;
            errorCode: string | null;
            /**
             * Format: int64
             * @description Где в файле начинается превью Deezer.
             */
            offsetMs: number | null;
            /**
             * Format: double
             * @description 0..1, порог — 0,8.
             */
            similarity: number | null;
            /** @description Совпал ли отпечаток с превью Deezer; `null` — проверить было нечем. */
            verified: boolean | null;
        };
        /** @description Что умеет загрузчик. */
        Capabilities: {
            /**
             * @description Есть cookies аккаунта Google.
             * @default false
             */
            cookies: boolean;
            /**
             * @description Виды заданий; пусто — любые, которые знает сервер.
             * @default []
             */
            kinds: string[];
            /**
             * @description Клиенты плеера YouTube; пусто — выбор yt-dlp.
             * @default []
             */
            playerClients: string[];
            /**
             * @description Есть сервис PO-токенов.
             * @default false
             */
            potProvider: boolean;
            /**
             * @description Версия yt-dlp.
             * @default null
             */
            ytDlp: string | null;
        };
        /** @description Наша сущность для id Deezer: поле `type` говорит, какая. */
        CatalogEntity: (components["schemas"]["ArtistSummary"] & {
            /** @enum {string} */
            type: "artist";
        }) | (components["schemas"]["AlbumSummary"] & {
            /** @enum {string} */
            type: "album";
        }) | (components["schemas"]["Track"] & {
            /** @enum {string} */
            type: "track";
        });
        /**
         * @description Запрос задания. Пока заданий нет, сервер держит запрос до ~25 секунд, затем отвечает 204
         *     с `Retry-After` — через сколько секунд спросить снова.
         */
        ClaimRequest: {
            capabilities?: components["schemas"]["Capabilities"];
            /** @description Версия fetcher. */
            version?: string | null;
            /** @description Имя машины, например `dalynkaa-mac`. */
            workerId: string;
        };
        ClaimResponse: {
            job: components["schemas"]["FetcherJob"];
        };
        /** @description Сообщение клиента. */
        ClientMessage: components["schemas"]["Ping"] & {
            /** @enum {string} */
            type: "ping";
        };
        /**
         * @description Откуда запущено.
         * @enum {string}
         */
        ContextType: "album" | "playlist" | "artist" | "wave" | "radio" | "mix" | "search" | "library" | "queue" | "remote";
        DayStats: {
            /** Format: int64 */
            acquisitionsFailed: number;
            /** Format: int64 */
            acquisitionsReady: number;
            /** Format: int64 */
            jobsDone: number;
            /** Format: int64 */
            jobsFailed: number;
            /** Format: int64 */
            likes: number;
            /** Format: int64 */
            newTracks: number;
            /** Format: int64 */
            plays: number;
        };
        /** @description Устройство с приложением: у каждого свой токен. */
        Device: {
            appVersion: string | null;
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            id: string;
            /** Format: date-time */
            lastSeenAt: string | null;
            name: string;
            platform: components["schemas"]["Platform"];
        };
        /** @description Что докачивать на сервер по подписке на артиста. */
        DownloadPolicy: {
            albums: boolean;
            /** @description Релизы, где главный артист альбома — другой (совместные, «участие»). */
            appearsOn: boolean;
            compilations: boolean;
            eps: boolean;
            /** @description Концертные записи (по названию релиза). */
            live: boolean;
            singles: boolean;
        };
        /** @description Домашний загрузчик в разделе «Источники». */
        Downloader: {
            capabilities: components["schemas"]["Capabilities"];
            /** Format: date-time */
            createdAt: string;
            /**
             * Format: int64
             * @description Сбои за сутки: повторы и окончательные.
             */
            failed24h: number;
            /** Format: uuid */
            id: string;
            /** Format: date-time */
            lastClaimAt: string | null;
            /**
             * Format: date-time
             * @description Последнее обращение: запрос задания, продление аренды, отчёт.
             */
            lastSeenAt: string | null;
            name: string;
            /**
             * Format: int64
             * @description Принятые файлы за сутки.
             */
            ok24h: number;
            /** @description Обращался в последние две минуты. */
            online: boolean;
            /** Format: date-time */
            revokedAt: string | null;
            /**
             * Format: int64
             * @description Задания в работе прямо сейчас.
             */
            running: number;
            /** @description Версия fetcher. */
            version: string | null;
            /** @description Имя машины из последнего запроса. */
            workerId: string | null;
            ytDlpVersion: string | null;
        };
        DownloaderList: {
            items: components["schemas"]["Downloader"][];
        };
        /** @description Новый загрузчик. Токен показывается один раз: на сервере остаётся только его хеш. */
        DownloaderToken: {
            downloader: components["schemas"]["Downloader"];
            /** @description `dwk_…`: `fetcher worker --token` или файл `~/.config/dalywave/downloader-token`. */
            token: string;
        };
        /**
         * @description Почему прослушивание закончилось.
         * @enum {string}
         */
        EndReason: "completed" | "skipped" | "previous" | "stopped" | "error" | "interrupted" | "transferred";
        /** @description Тело любой ошибки API: `{ "error": { "code": "...", "message": "..." } }`. */
        ErrorBody: {
            error: components["schemas"]["ErrorInfo"];
        };
        ErrorInfo: {
            /** @description Машинный код в snake_case: по нему интерфейс выбирает текст и действие. */
            code: string;
            /** @description Понятный текст по-русски. */
            message: string;
        };
        /** @description Пачка событий из приложения: до 500 каждого вида. */
        EventBatch: {
            feedback?: components["schemas"]["FeedbackEvent"][];
            plays?: components["schemas"]["PlayEvent"][];
        };
        /** @description Итог приёма. Принятые и повторы приложение может удалить у себя; отклонённые повторять бессмысленно. */
        EventResult: {
            /** Format: int32 */
            accepted: number;
            /**
             * Format: int32
             * @description Уже были приняты раньше.
             */
            duplicates: number;
            rejected: components["schemas"]["RejectedEvent"][];
        };
        /** @description Грань вкуса с примерами. */
        FacetView: {
            /** @description Поправки по времени суток (`timeOfDay`) и устройству (`platform`): больше 1 — грань слушают чаще. */
            context: Record<string, never>;
            examples: components["schemas"]["Track"][];
            /** Format: uuid */
            id: string;
            label: string;
            /** Format: int32 */
            position: number;
            /** Format: int32 */
            tracks: number;
            /** Format: date-time */
            updatedAt: string;
            /** Format: uuid */
            userId: string;
            userName: string;
            /**
             * Format: float
             * @description Доля вкуса, 0..1.
             */
            weight: number;
        };
        FeedbackEvent: {
            /** Format: uuid */
            artistId?: string | null;
            /** Format: date-time */
            at: string;
            /** Format: uuid */
            clientEventId: string;
            contextId?: string | null;
            contextType?: components["schemas"]["ContextType"] | null;
            kind: components["schemas"]["FeedbackKind"];
            /** Format: uuid */
            trackId?: string | null;
            /** Format: uuid */
            waveSessionId?: string | null;
        };
        /** @enum {string} */
        FeedbackKind: "like" | "unlike" | "dislike" | "undislike" | "not_now" | "more_like_this" | "playlist_add" | "queue_add" | "replay";
        /** @description Задание загрузчику. */
        FetcherJob: {
            /** Format: int64 */
            deezerTrackId: number;
            /** @description Прямая ссылка из «Спорных» (SoundCloud, Bandcamp, …) вместо поиска. */
            directUrl: string | null;
            excludeVideoIds: string[];
            /**
             * Format: uuid
             * @description id задачи очереди: с ним отчёт и продление аренды.
             */
            id: string;
            /** @description Всегда `acquire`. */
            kind: string;
            /**
             * Format: date-time
             * @description Не пришёл результат к этому сроку — задание вернётся в очередь. Продлевается `…/lease`.
             */
            leaseExpiresAt: string;
            /**
             * Format: int32
             * @description Сколько кандидатов пробовать скачать.
             */
            maxAttempts: number;
            /** @description Видео, выбранные вручную в «Спорных»: пробуются первыми и принимаются без совпадения отпечатка. */
            preferVideoIds: string[];
            upload: components["schemas"]["UploadTarget"];
        };
        /** @description Подписаться или изменить подписку: только переданные поля. */
        FollowRequest: {
            downloadPolicy?: components["schemas"]["DownloadPolicy"] | null;
            /** @description Сообщать о новых релизах. */
            notify?: boolean | null;
        };
        /** @description Подписка на артиста. */
        FollowState: {
            /** Format: date-time */
            followedAt: string;
            notify: boolean;
            policy: components["schemas"]["DownloadPolicy"];
        };
        FollowedArtist: {
            artist: components["schemas"]["ArtistSummary"];
            /** Format: date-time */
            followedAt: string;
            notify: boolean;
            policy: components["schemas"]["DownloadPolicy"];
            /**
             * Format: int64
             * @description Сколько треков дискографии на сервере и сколько всего.
             */
            tracksOnServer: number;
            /** Format: int64 */
            tracksTotal: number;
        };
        FollowedArtistPage: {
            items: components["schemas"]["FollowedArtist"][];
            nextCursor: string | null;
        };
        Health: {
            db: components["schemas"]["HealthStatus"];
            status: components["schemas"]["HealthStatus"];
            /**
             * Format: int64
             * @description Сколько секунд работает процесс API.
             */
            uptimeS: number;
            version: string;
        };
        /** @enum {string} */
        HealthStatus: "ok" | "error";
        Hello: {
            /**
             * Format: int32
             * @description Как часто сервер шлёт ping-кадры WebSocket. Без ответа за два интервала соединение закрывается.
             */
            heartbeatMs: number;
            /** Format: date-time */
            serverTime: string;
            /** Format: uuid */
            sessionId: string;
            /** Format: uuid */
            userId: string;
        };
        HistoryItem: {
            contextId: string | null;
            contextType: components["schemas"]["ContextType"] | null;
            endReason: components["schemas"]["EndReason"];
            /** Format: uuid */
            id: string;
            /** Format: int32 */
            playedMs: number;
            /** Format: date-time */
            startedAt: string;
            track: components["schemas"]["Track"];
        };
        HistoryPage: {
            items: components["schemas"]["HistoryItem"][];
            nextCursor: string | null;
        };
        /** @description Пустые полки не приходят. */
        Home: {
            /** Format: date-time */
            generatedAt: string;
            shelves: components["schemas"]["Shelf"][];
        };
        /** @enum {string} */
        ImageOwner: "album" | "artist";
        /** @description Картинка в трёх размерах. */
        ImageSet: {
            /** @description 1000×1000. */
            large: string;
            /** @description 500×500. */
            medium: string;
            /** @description 250×250. */
            small: string;
        };
        /** @description Показ рекомендации и чем он кончился. */
        ImpressionView: {
            /** Format: uuid */
            id: string;
            /** @description `like`, `dislike`, `skip`, `not_now`, `more_like_this`, `completed`; `null` — реакции не было. */
            outcome: string | null;
            reason: components["schemas"]["Reason"] | null;
            /** Format: float */
            score: number | null;
            /** Format: date-time */
            shownAt: string;
            /** @description `wave`, `radio`, `similar`, `mix`, `discovery`. */
            surface: string;
            track: components["schemas"]["Track"];
            userName: string;
            /** Format: uuid */
            waveSessionId: string | null;
        };
        IndexStats: {
            /**
             * Format: int64
             * @description За сутки: проанализировано (полностью и по превью) и сбоев.
             */
            analyzed24h: number;
            /**
             * Format: int64
             * @description Артистов в окрестности вкуса; из них семян.
             */
            artists: number;
            bySource: components["schemas"]["ScoutSourceStats"][];
            /** Format: int64 */
            failed24h: number;
            /**
             * Format: int64
             * @description Полных анализов скачанного.
             */
            full: number;
            /** Format: int64 */
            maxIndex: number;
            /**
             * Format: int64
             * @description Задачи в очереди.
             */
            queuedFull: number;
            /** Format: int64 */
            queuedPreview: number;
            /**
             * Format: int64
             * @description Записей на сервере без полного анализа.
             */
            readyWithoutAnalysis: number;
            /** Format: int64 */
            seeds: number;
            /**
             * Format: int64
             * @description Анализов по превью — это и есть индекс каталога.
             */
            size: number;
        };
        Job: {
            /** Format: int32 */
            attempts: number;
            /** Format: date-time */
            createdAt: string;
            dedupeKey: string | null;
            /** Format: date-time */
            finishedAt: string | null;
            /** Format: date-time */
            heartbeatAt: string | null;
            /** Format: uuid */
            id: string;
            kind: string;
            lane: string;
            lastError: string | null;
            /**
             * Format: date-time
             * @description Аренда у внешнего исполнителя (загрузчика): не отчитался к сроку — задача снова в очереди.
             */
            leaseExpiresAt: string | null;
            /** @description Исполнитель: `<host>:<pid>` воркера или `device:<id>` домашнего загрузчика. */
            lockedBy: string | null;
            /** Format: int32 */
            maxAttempts: number;
            payload: Record<string, never>;
            /**
             * Format: int32
             * @description Больше — раньше.
             */
            priority: number;
            result: Record<string, never> | null;
            /**
             * Format: date-time
             * @description Не раньше этого времени (повтор с задержкой).
             */
            runAfter: string;
            status: components["schemas"]["JobStatus"];
            /** Format: date-time */
            updatedAt: string;
        };
        JobPage: {
            items: components["schemas"]["Job"][];
            nextCursor: string | null;
        };
        /** @enum {string} */
        JobStatus: "queued" | "running" | "done" | "failed" | "cancelled";
        JobUpdate: {
            /** Format: int32 */
            attempts: number;
            /** Format: uuid */
            id: string;
            kind: string;
            lane: string;
            lastError: string | null;
            /** Format: int32 */
            maxAttempts: number;
            /** Format: date-time */
            runAfter: string;
            status: components["schemas"]["JobStatus"];
        };
        /** @description Полоса очереди со счётчиками. */
        Lane: {
            /** Format: int32 */
            concurrency: number;
            /**
             * Format: int64
             * @description Выполнено за сутки.
             */
            done24h: number;
            /** Format: int64 */
            failed: number;
            lane: string;
            /**
             * Format: int32
             * @description Пауза между стартами задач полосы, мс.
             */
            minIntervalMs: number;
            paused: boolean;
            /** Format: int64 */
            queued: number;
            /** Format: int64 */
            running: number;
            title: string;
            /** Format: date-time */
            updatedAt: string;
        };
        LaneList: {
            items: components["schemas"]["Lane"][];
        };
        /** @description Изменение полосы: только переданные поля. */
        LanePatch: {
            /**
             * Format: int32
             * @description 1–16.
             */
            concurrency?: number | null;
            /**
             * Format: int32
             * @description 0–3 600 000.
             */
            minIntervalMs?: number | null;
            paused?: boolean | null;
        };
        Lease: {
            /** Format: date-time */
            leaseExpiresAt: string;
        };
        /** @description Продление аренды: загрузчик ещё работает над заданием. */
        LeaseRequest: {
            workerId?: string | null;
        };
        LibraryChange: {
            /** @description Какие таблицы локальной базы затронуты. */
            entities: components["schemas"]["SyncEntity"][];
        };
        LibraryCounts: {
            /** Format: int64 */
            followedArtists: number;
            /** Format: int64 */
            likedTracks: number;
            /** Format: int64 */
            playlists: number;
            /**
             * Format: int64
             * @description Треки медиатеки, которые уже на сервере.
             */
            readyTracks: number;
            /** Format: int64 */
            savedAlbums: number;
        };
        /** @description Начальный экран медиатеки. */
        LibraryOverview: {
            counts: components["schemas"]["LibraryCounts"];
            /** @description Подписки, недавние первыми (до 50). */
            followedArtists: components["schemas"]["FollowedArtist"][];
            pinnedPlaylists: components["schemas"]["PlaylistSummary"][];
        };
        LibraryStats: {
            /** Format: int64 */
            albums: number;
            /** Format: int64 */
            artists: number;
            /** Format: int64 */
            followedArtists: number;
            /** Format: int64 */
            likedTracks: number;
            /** Format: int64 */
            playlists: number;
            /** Format: int64 */
            recordings: number;
            /** Format: int64 */
            savedAlbums: number;
            /** Format: int64 */
            tracks: number;
        };
        LikedTrack: {
            /** Format: date-time */
            likedAt: string;
            track: components["schemas"]["Track"];
        };
        LikedTrackPage: {
            items: components["schemas"]["LikedTrack"][];
            nextCursor: string | null;
        };
        /** @description Кто вошёл. Отвечает и людям без ролей: интерфейс показывает им «ждём доступа». */
        Me: {
            device: components["schemas"]["Device"] | null;
            permissions: components["schemas"]["Permission"][];
            roles: components["schemas"]["Role"][];
            /** Format: uuid */
            sessionId: string;
            user: components["schemas"]["User"];
            via: components["schemas"]["AuthVia"];
        };
        MirrorImagePayload: {
            /** @description Скопировать заново, даже если копия есть. */
            force?: boolean;
            /** Format: uuid */
            id: string;
            owner: components["schemas"]["ImageOwner"];
        };
        /** @description Сервис анализа звука. */
        MlStatus: {
            error: string | null;
            /** @description Ответ `/v1/healthz`: загружены ли модели, занят ли сервис, версии. */
            health: Record<string, never> | null;
            reachable: boolean;
            url: string;
        };
        /** @description Переставить позицию: после другой или в начало (`afterItemId: null`). */
        MovePlaylistItem: {
            /** Format: uuid */
            afterItemId?: string | null;
        };
        MovedPlaylistItem: {
            /** Format: uuid */
            id: string;
            /** Format: double */
            position: number;
            /** @description Позиции остальных треков пересчитаны: интерфейсу стоит перечитать плейлист. */
            rebalanced: boolean;
        };
        NewDownloader: {
            /** @description Например «Mac Захара». */
            name: string;
        };
        NewPlaylist: {
            description?: string | null;
            title: string;
            /** @description Треки в этом порядке. */
            trackIds?: string[];
        };
        NewWaveSession: {
            settings?: components["schemas"]["WaveSettings"];
        };
        OidcProvider: {
            /** @description Подпись кнопки входа, например `daly.rest`. */
            title: string;
        };
        PasswordLogin: {
            password: string;
        };
        /** @enum {string} */
        Permission: "play" | "library" | "admin" | "downloader";
        Ping: {
            id?: string | null;
        };
        /** @enum {string} */
        Platform: "web" | "macos" | "windows" | "linux" | "ios" | "ipados" | "android";
        /** @description Одно прослушивание. */
        PlayEvent: {
            /** Format: uuid */
            clientEventId: string;
            contextId?: string | null;
            contextType?: components["schemas"]["ContextType"] | null;
            /**
             * Format: int32
             * @description Длительность трека у плеера.
             */
            durationMs: number;
            endReason: components["schemas"]["EndReason"];
            /** @description Выход звука: `headphones`, `speaker`, `bluetooth`, `airplay`. */
            output?: string | null;
            /**
             * Format: int32
             * @description Сколько реально сыграно, без перемоток.
             */
            playedMs: number;
            /**
             * Format: int32
             * @description С какой позиции началось.
             */
            positionStartMs?: number;
            source?: components["schemas"]["PlaySource"];
            /** Format: date-time */
            startedAt: string;
            /** Format: uuid */
            trackId: string;
            /** @description Режим перехода к следующему треку: `gapless`, `crossfade`, `smart`, `dj`. */
            transition?: string | null;
            /** Format: uuid */
            waveSessionId?: string | null;
        };
        /** @enum {string} */
        PlaySource: "stream" | "offline";
        PlaylistDetail: {
            items: components["schemas"]["PlaylistItem"][];
            playlist: components["schemas"]["PlaylistSummary"];
        };
        PlaylistItem: {
            /** Format: date-time */
            addedAt: string;
            /** Format: uuid */
            id: string;
            /**
             * Format: double
             * @description Дробная позиция: порядок — по возрастанию.
             */
            position: number;
            track: components["schemas"]["Track"];
        };
        /** @enum {string} */
        PlaylistKind: "user" | "smart" | "mix" | "system";
        PlaylistList: {
            items: components["schemas"]["PlaylistSummary"][];
        };
        PlaylistPatch: {
            description?: string | null;
            title?: string | null;
        };
        PlaylistSummary: {
            /** @description Обложки первых альбомов плейлиста (до четырёх) — для мозаики. */
            covers: components["schemas"]["ImageSet"][];
            /** Format: date-time */
            createdAt: string;
            description: string;
            /** Format: int64 */
            durationMs: number;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["PlaylistKind"];
            pinned: boolean;
            title: string;
            /** Format: int64 */
            tracksCount: number;
            /** Format: date-time */
            updatedAt: string;
        };
        Pong: {
            id: string | null;
        };
        /** @description Подпись под названием трека: вид для иконки и готовый текст по-русски. */
        Reason: {
            kind: components["schemas"]["ReasonKind"];
            text: string;
        };
        /**
         * @description Почему трек здесь.
         * @enum {string}
         */
        ReasonKind: "similar" | "session" | "seed" | "query" | "new_to_you" | "forgotten" | "favorite" | "often" | "new_release" | "popular" | "rare";
        /** @description Раздел «Рекомендации». */
        RecoOverview: {
            /** @description Грани вкуса, самые весомые первыми. */
            facets: components["schemas"]["FacetView"][];
            /** @description Последние показы, новые первыми. */
            impressions: components["schemas"]["ImpressionView"][];
            index: components["schemas"]["IndexStats"];
            ml: components["schemas"]["MlStatus"];
            /** @description Версия, которая собирает поток: `rules-1`, пока своей модели нет. */
            modelVersion: string | null;
            settings: components["schemas"]["RecoSettings"];
        };
        RecoRun: {
            task: components["schemas"]["RecoTask"];
        };
        RecoRunResult: {
            /**
             * Format: int64
             * @description Сколько задач поставлено (уже стоящие не считаются).
             */
            queued: number;
        };
        /** @description Настройки разведчика. Остановить анализ или разведку — пауза полос `analysis` и `scout`. */
        RecoSettings: {
            /**
             * Format: float
             * @description Во сколько раз падает близость на каждом шаге (0..1).
             */
            hopDecay: number;
            /**
             * Format: int32
             * @description Сколько шагов по похожим артистам Deezer (0–3).
             */
            maxHops: number;
            /**
             * Format: int32
             * @description Предел индекса: анализов по превью; дальние от вкуса вытесняются.
             */
            maxIndex: number;
            /**
             * Format: int32
             * @description Сколько треков в сутки разведчик отдаёт на анализ превью.
             */
            tracksPerDay: number;
            /** Format: date-time */
            updatedAt: string;
            /**
             * Format: int32
             * @description Часовой пояс слушателя в минутах от UTC: время суток у граней вкуса.
             */
            utcOffsetMin: number;
        };
        /** @description Изменение настроек: только переданные поля. */
        RecoSettingsPatch: {
            /**
             * Format: float
             * @description 0.05–0.95.
             */
            hopDecay?: number | null;
            /**
             * Format: int32
             * @description 0–3.
             */
            maxHops?: number | null;
            /**
             * Format: int32
             * @description 100–2 000 000.
             */
            maxIndex?: number | null;
            /**
             * Format: int32
             * @description 0–50 000.
             */
            tracksPerDay?: number | null;
            /**
             * Format: int32
             * @description −720…840.
             */
            utcOffsetMin?: number | null;
        };
        /**
         * @description Что запустить сейчас, не дожидаясь расписания.
         * @enum {string}
         */
        RecoTask: "scout" | "taste" | "analysis";
        /**
         * @description Тип релиза у Deezer.
         * @enum {string}
         */
        RecordType: "album" | "ep" | "single" | "compile";
        RefreshArtistPayload: {
            /** Format: uuid */
            artistId: string;
            /** @description Обновить всё, даже свежее. */
            force?: boolean;
        };
        RejectedEvent: {
            /** Format: uuid */
            clientEventId: string;
            /** @description Машинная причина: `unknown_track`, `unknown_artist`, `invalid`. */
            reason: string;
        };
        /**
         * @description Загрузчик бросает задание (приложение закрывают посреди скачивания): задание сразу возвращается
         *     в очередь, попытка не тратится.
         */
        ReleaseRequest: {
            /** @description Почему: `quit`, `pause`, `control` — для журнала задачи. */
            reason?: string | null;
            workerId?: string | null;
        };
        /** @description Дискография по типам релизов, новые первыми. */
        Releases: {
            albums: components["schemas"]["AlbumSummary"][];
            compilations: components["schemas"]["AlbumSummary"][];
            eps: components["schemas"]["AlbumSummary"][];
            singles: components["schemas"]["AlbumSummary"][];
        };
        /**
         * @description Что сделать со спорной (или неудачной) записью.
         * @enum {string}
         */
        ResolveAction: "choose" | "url" | "unavailable" | "retry";
        ResolveRequest: {
            action: components["schemas"]["ResolveAction"];
            /** @description Для `url`. */
            url?: string | null;
            /** @description Для `choose`: id видео YouTube (11 символов). */
            videoId?: string | null;
        };
        /**
         * @description Отчёт о задании: `result` — тот же объект, что отдаёт `POST /v1/acquire` сервиса fetcher
         *     (статус, выбранный кандидат, файл с громкостью и формой волны, попытки, кандидаты, ошибка).
         */
        ResultReport: {
            result: Record<string, never>;
            workerId: string;
        };
        Resync: {
            /** @description Почему: `lagged` — клиент не успевал читать события. */
            reason: string;
        };
        /**
         * @description Роль в DalyWave. Роли совмещаются; человек без ролей видит экран «ждём доступа».
         * @enum {string}
         */
        Role: "admin" | "listener";
        SavedAlbum: {
            album: components["schemas"]["AlbumSummary"];
            /** Format: date-time */
            savedAt: string;
        };
        SavedAlbumPage: {
            items: components["schemas"]["SavedAlbum"][];
            nextCursor: string | null;
        };
        ScoutArtistPayload: {
            /** Format: uuid */
            artistId: string;
        };
        /** @description Сколько найдено и проанализировано по виду находки разведчика. */
        ScoutSourceStats: {
            /** Format: int64 */
            analyzed: number;
            /** Format: int64 */
            found: number;
            /** Format: int64 */
            pending: number;
            /** @description `top`, `release`, `radio`, `chart`, `editorial`. */
            source: string;
        };
        SearchResults: {
            albums: components["schemas"]["AlbumSummary"][];
            artists: components["schemas"]["ArtistSummary"][];
            scope: components["schemas"]["SearchScope"];
            tracks: components["schemas"]["Track"][];
        };
        /** @enum {string} */
        SearchScope: "catalog" | "library";
        /**
         * @description От чего радио.
         * @enum {string}
         */
        SeedKind: "track" | "artist" | "album" | "playlist" | "text";
        /** @description Сообщение сервера. */
        ServerEvent: (components["schemas"]["Hello"] & {
            /** @enum {string} */
            type: "hello";
        }) | (components["schemas"]["Pong"] & {
            /** @enum {string} */
            type: "pong";
        }) | (components["schemas"]["AcquisitionUpdate"] & {
            /** @enum {string} */
            type: "acquisition.updated";
        }) | (components["schemas"]["JobUpdate"] & {
            /** @enum {string} */
            type: "job.updated";
        }) | (components["schemas"]["LibraryChange"] & {
            /** @enum {string} */
            type: "library.changed";
        }) | (components["schemas"]["Resync"] & {
            /** @enum {string} */
            type: "resync";
        });
        /** @description Серверный fetcher — второй исполнитель (`DALYWAVE_FETCHER_MODE` с `server`). */
        ServerFetcher: {
            enabled: boolean;
            error: string | null;
            /** Format: int64 */
            failed24h: number;
            /** @description Ответ `/v1/healthz`: версии yt-dlp, Deno, ffmpeg, сервис PO-токенов, срок cookies. */
            health: Record<string, never> | null;
            /** Format: int64 */
            ok24h: number;
            /** @description Ответил ли `/v1/healthz`; `null`, если серверный fetcher выключен. */
            reachable: boolean | null;
            /** Format: int64 */
            running: number;
            url: string | null;
        };
        /** @description Открытая сессия: вкладка сайта или приложение. */
        SessionInfo: {
            /** Format: date-time */
            createdAt: string;
            /** @description Это сессия, из которой сделан запрос. */
            current: boolean;
            device: components["schemas"]["Device"] | null;
            /** Format: date-time */
            expiresAt: string;
            /** Format: uuid */
            id: string;
            ip: string | null;
            kind: components["schemas"]["SessionKind"];
            /** Format: date-time */
            lastSeenAt: string;
            userAgent: string | null;
            /** Format: uuid */
            userId: string;
            userName: string;
            via: components["schemas"]["AuthVia"];
        };
        /** @enum {string} */
        SessionKind: "web" | "device";
        SessionList: {
            items: components["schemas"]["SessionInfo"][];
            nextCursor: string | null;
        };
        Shelf: {
            /**
             * @description Постоянный id полки: `continue`, `daily-mixes`, `new-releases`, `discover-weekly`, `recent-likes`,
             *     `forgotten-favorites`, `top-artists`.
             */
            id: string;
            items: components["schemas"]["ShelfItem"][];
            kind: components["schemas"]["ShelfKind"];
            /** @description Заголовок по-русски; английский интерфейс берёт свой по `id`. */
            title: string;
        };
        /** @description Элемент полки: поле `type` говорит, что внутри. */
        ShelfItem: (components["schemas"]["Track"] & {
            /** @enum {string} */
            type: "track";
        }) | (components["schemas"]["AlbumSummary"] & {
            /** @enum {string} */
            type: "album";
        }) | (components["schemas"]["ArtistSummary"] & {
            /** @enum {string} */
            type: "artist";
        }) | (components["schemas"]["PlaylistSummary"] & {
            /** @enum {string} */
            type: "playlist";
        });
        /** @enum {string} */
        ShelfKind: "continue" | "new_releases" | "recent_likes" | "top_artists" | "daily_mixes" | "discover_weekly" | "forgotten_favorites";
        /** @description Поиск словами по звучанию: «грустный инди под дождь». */
        SoundSearch: {
            /**
             * Format: int32
             * @description Сколько треков в каждом списке: 1–50, по умолчанию 20.
             */
            limit?: number | null;
            text: string;
        };
        SoundSearchResults: {
            /** @description Ближайшее из индекса каталога: то, что ещё не слушал. */
            catalog: components["schemas"]["Track"][];
            /** @description Ближайшее по звучанию из медиатеки: лайки, плейлисты, прослушанное, скачанное. */
            library: components["schemas"]["Track"][];
            /** @description Запрос, как его понял сервис анализа (английский для CLAP). */
            query: string;
            /** @description Запрос переведён словарём. */
            translated: boolean;
            /** @description Незнакомые словарю слова: в поиск они не попали. */
            unknownWords: string[];
        };
        /** @description Раздел «Источники»: кто качает аудио. */
        Sources: {
            downloaders: components["schemas"]["Downloader"][];
            server: components["schemas"]["ServerFetcher"];
        };
        StatusCount: {
            /** Format: int64 */
            count: number;
            status: components["schemas"]["AudioStatus"];
        };
        StorageStats: {
            /** Format: int64 */
            audioBytes: number;
            /** Format: int64 */
            audioFiles: number;
            /** Format: int64 */
            imageBytes: number;
            /**
             * Format: int64
             * @description Скопированные обложки и фото (каждая — три размера).
             */
            images: number;
        };
        /** @description Аудио ещё нет: скачивание стоит в очереди или идёт. */
        StreamPending: {
            /**
             * Format: uuid
             * @description Следить за ним — `GET /api/acquisitions/{id}` или событие `acquisition.updated`.
             */
            acquisitionId: string;
            /**
             * Format: int64
             * @description Загрузчики на связи.
             */
            downloadersOnline: number;
            /**
             * Format: int64
             * @description Сколько скачиваний впереди; 0 — следующее или уже идёт.
             */
            queuePosition: number;
        };
        /** @description Аудио готово: ссылка на файл и всё, что нужно плееру без лишних запросов. */
        StreamReady: {
            /** Format: int32 */
            bitrateKbps: number | null;
            codec: components["schemas"]["AudioCodec"];
            container: components["schemas"]["AudioContainer"];
            /** Format: int32 */
            durationMs: number;
            /**
             * Format: date-time
             * @description Ссылка перестаёт работать в это время (через 6 часов): потом — запросить поток снова.
             */
            expiresAt: string;
            /** Format: uuid */
            fileId: string;
            /**
             * Format: float
             * @description Интегральная громкость (EBU R128) — для выравнивания.
             */
            loudnessLufs: number | null;
            /** @description Для `<audio>` и `MediaSource`: `audio/mp4; codecs="opus"`. */
            mimeType: string;
            /** Format: float */
            truePeakDb: number | null;
            /** @description Подписанная ссылка на файл в хранилище. */
            url: string;
            /** @description Огибающая для полосы прокрутки: 512 точек, 0..1000. */
            waveform: number[] | null;
        };
        SyncAlbum: {
            /** Format: uuid */
            artistId: string;
            /** Format: int64 */
            deezerId: number | null;
            /** Format: int32 */
            durationMs: number | null;
            explicit: boolean;
            genres: string[];
            /** Format: uuid */
            id: string;
            image: components["schemas"]["ImageSet"] | null;
            label: string | null;
            palette: string[] | null;
            recordType: components["schemas"]["RecordType"];
            /** Format: date */
            releaseDate: string | null;
            /** Format: int64 */
            rev: number;
            title: string;
            /** Format: int32 */
            tracksCount: number | null;
        };
        SyncArtist: {
            /** Format: int64 */
            deezerId: number | null;
            /** Format: uuid */
            id: string;
            image: components["schemas"]["ImageSet"] | null;
            name: string;
            /** Format: int64 */
            rev: number;
        };
        SyncBlockedItem: {
            /** Format: date-time */
            createdAt: string;
            kind: components["schemas"]["BlockedKind"];
            /** Format: uuid */
            refId: string;
            /** Format: int64 */
            rev: number;
        };
        SyncDeviceDownload: {
            /** Format: uuid */
            deviceId: string;
            /** Format: uuid */
            id: string;
            /** @description `track`, `album`, `playlist`, `artist` или `liked`. */
            kind: string;
            /** Format: uuid */
            refId: string | null;
            /** Format: int64 */
            rev: number;
        };
        SyncDiscographyPayload: {
            /** Format: uuid */
            artistId: string;
        };
        /**
         * @description Таблица локальной базы.
         * @enum {string}
         */
        SyncEntity: "artists" | "albums" | "tracks" | "recordings" | "likedTracks" | "savedAlbums" | "followedArtists" | "blockedItems" | "playlists" | "playlistItems" | "deviceDownloads";
        SyncFollowedArtist: {
            /** Format: uuid */
            artistId: string;
            /** Format: date-time */
            followedAt: string;
            notify: boolean;
            policy: components["schemas"]["DownloadPolicy"];
            /** Format: int64 */
            rev: number;
        };
        SyncLikedTrack: {
            /** Format: date-time */
            likedAt: string;
            /** Format: int64 */
            rev: number;
            /** Format: uuid */
            trackId: string;
        };
        SyncPage: {
            albums: components["schemas"]["SyncAlbum"][];
            artists: components["schemas"]["SyncArtist"][];
            blockedItems: components["schemas"]["SyncBlockedItem"][];
            /** @description Передать в `since` в следующий раз. */
            cursor: string;
            deviceDownloads: components["schemas"]["SyncDeviceDownload"][];
            followedArtists: components["schemas"]["SyncFollowedArtist"][];
            /** @description Есть ещё изменения: запросить следующую страницу сразу. */
            hasMore: boolean;
            likedTracks: components["schemas"]["SyncLikedTrack"][];
            playlistItems: components["schemas"]["SyncPlaylistItem"][];
            playlists: components["schemas"]["SyncPlaylist"][];
            recordings: components["schemas"]["SyncRecording"][];
            savedAlbums: components["schemas"]["SyncSavedAlbum"][];
            tombstones: components["schemas"]["SyncTombstone"][];
            tracks: components["schemas"]["SyncTrack"][];
        };
        SyncPlaylist: {
            /** Format: date-time */
            createdAt: string;
            description: string;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["PlaylistKind"];
            pinned: boolean;
            /** Format: int64 */
            rev: number;
            title: string;
            /** Format: date-time */
            updatedAt: string;
        };
        SyncPlaylistItem: {
            /** Format: date-time */
            addedAt: string;
            /** Format: uuid */
            id: string;
            /** Format: uuid */
            playlistId: string;
            /** Format: double */
            position: number;
            /** Format: int64 */
            rev: number;
            /** Format: uuid */
            trackId: string;
        };
        SyncRecording: {
            audioStatus: components["schemas"]["AudioStatus"];
            /** Format: int32 */
            durationMs: number;
            /** Format: uuid */
            id: string;
            isrc: string | null;
            /** Format: uuid */
            primaryFileId: string | null;
            /** Format: int64 */
            rev: number;
        };
        SyncSavedAlbum: {
            /** Format: uuid */
            albumId: string;
            /** Format: int64 */
            rev: number;
            /** Format: date-time */
            savedAt: string;
        };
        /** @description Удаление строки. `key` — ключ строки в camelCase, например `{ "trackId": … }` у `likedTracks`. */
        SyncTombstone: {
            /** Format: date-time */
            deletedAt: string;
            entity: components["schemas"]["SyncEntity"];
            key: Record<string, never>;
            /** Format: int64 */
            rev: number;
        };
        SyncTrack: {
            /** Format: uuid */
            albumId: string | null;
            /** @description По порядку: основные, затем приглашённые. */
            artists: components["schemas"]["SyncTrackArtist"][];
            /** Format: int64 */
            deezerId: number | null;
            /** Format: int32 */
            discNumber: number;
            /** Format: int32 */
            durationMs: number;
            explicit: boolean;
            /** Format: uuid */
            id: string;
            isrc: string | null;
            /** Format: uuid */
            recordingId: string | null;
            /** Format: int64 */
            rev: number;
            title: string;
            /** Format: int32 */
            trackPosition: number | null;
            version: string;
        };
        SyncTrackArtist: {
            /** Format: uuid */
            artistId: string;
            role: components["schemas"]["ArtistRole"];
        };
        TastePayload: {
            /** @description Собрать «Открытия недели» заново, даже если им нет недели. */
            refreshDiscovery?: boolean;
            /** Format: uuid */
            userId: string;
        };
        /** @description Трек каталога — позиция в альбоме. Одна запись (ISRC) может быть у нескольких треков. */
        Track: {
            album: components["schemas"]["AlbumRef"] | null;
            /** @description Сначала основные артисты, затем приглашённые. */
            artists: components["schemas"]["TrackArtist"][];
            audioStatus: components["schemas"]["AudioStatus"];
            /** Format: int64 */
            deezerId: number | null;
            /** Format: int32 */
            discNumber: number;
            /** Format: int32 */
            durationMs: number;
            explicit: boolean;
            /** Format: uuid */
            id: string;
            isrc: string | null;
            liked: boolean;
            /**
             * Format: uuid
             * @description `null`, пока ISRC трека неизвестен.
             */
            recordingId: string | null;
            title: string;
            /** Format: int32 */
            trackPosition: number | null;
            /** @description Версия из названия у Deezer: «Slowed», «Remastered 2011». Пустая строка, если её нет. */
            version: string;
        };
        TrackArtist: {
            /** Format: uuid */
            id: string;
            name: string;
            role: components["schemas"]["ArtistRole"];
        };
        TrackList: {
            items: components["schemas"]["Track"][];
        };
        TrackLookup: {
            /** @description До 500 id; неизвестные пропускаются. */
            ids: string[];
        };
        /** @description Ответ `GET /api/tracks/{id}/stream`; поле `status` говорит, какой. */
        TrackStream: (components["schemas"]["StreamReady"] & {
            /** @enum {string} */
            status: "ready";
        }) | (components["schemas"]["StreamPending"] & {
            /** @enum {string} */
            status: "acquiring";
        }) | (components["schemas"]["StreamPending"] & {
            /** @enum {string} */
            status: "waiting_for_downloader";
        });
        /**
         * @description Куда положить файл: подписанная ссылка хранилища. Content-Type в подпись не входит —
         *     контейнер известен только после скачивания.
         */
        UploadTarget: {
            /**
             * Format: date-time
             * @description Ссылка живёт дольше аренды задания.
             */
            expiresAt: string;
            headers: {
                [key: string]: string;
            };
            /** @description Всегда `PUT`. */
            method: string;
            url: string;
        };
        /** @description Человек в DalyWave. */
        User: {
            avatarUrl: string | null;
            email: string | null;
            /** Format: uuid */
            id: string;
            name: string;
        };
        UserList: {
            items: components["schemas"]["AdminUser"][];
            nextCursor: string | null;
        };
        /** @description Изменение пользователя: только переданные поля. */
        UserPatch: {
            disabled?: boolean | null;
            /** @description Полный новый список ролей. */
            roles?: components["schemas"]["Role"][] | null;
        };
        /**
         * @description Занятие: задаёт границы энергии и темпа и ход сессии («Засыпаю» снижает энергию, «Просыпаюсь» — поднимает).
         * @enum {string}
         */
        WaveActivity: "any" | "wake" | "road" | "work" | "sport" | "rest" | "sleep" | "party";
        /**
         * @description Характер: сколько знакомого и нового, популярное или редкое.
         * @enum {string}
         */
        WaveCharacter: "fav" | "mix" | "new" | "pop" | "rare";
        WaveFeedback: {
            kind: components["schemas"]["WaveFeedbackKind"];
            /**
             * Format: int32
             * @description Где был трек в момент реакции: ранний пропуск весит больше позднего.
             */
            positionMs?: number | null;
            /** Format: uuid */
            trackId: string;
        };
        /**
         * @description Реакция на трек волны.
         * @enum {string}
         */
        WaveFeedbackKind: "like" | "dislike" | "skip" | "not_now" | "more_like_this" | "completed";
        /** @description Трек выдачи: почему он здесь и данные для стыков (темп, тональность по Камелоту, энергия). */
        WaveItem: {
            /** Format: float */
            bpm: number | null;
            /**
             * Format: float
             * @description 0..1.
             */
            energy: number | null;
            /** @description 8A, 11B … */
            keyCamelot: string | null;
            reason: components["schemas"]["Reason"];
            track: components["schemas"]["Track"];
        };
        WaveItems: {
            items: components["schemas"]["WaveItem"][];
        };
        /** @enum {string} */
        WaveLanguage: "any" | "ru" | "foreign" | "none";
        /** @enum {string} */
        WaveMood: "any" | "energetic" | "happy" | "calm" | "sad";
        /** @description Затравка: `id` — для трека, артиста, альбома и плейлиста, `text` — для описания словами. */
        WaveSeed: {
            /** Format: uuid */
            id?: string | null;
            text?: string | null;
            type: components["schemas"]["SeedKind"];
        };
        /** @description Сессия волны (или радио — с затравкой). */
        WaveSession: {
            /** Format: uuid */
            id: string;
            /**
             * Format: int32
             * @description Сколько треков уже выдано.
             */
            served: number;
            settings: components["schemas"]["WaveSettings"];
            /** Format: date-time */
            startedAt: string;
            /** @description Подпись: «Моя волна», «Радио: Lowborn», «По запросу «…»». */
            title: string;
        };
        /** @description Настройки волны; не переданные — по умолчанию: любое занятие, знакомое и новое, любое настроение и язык. */
        WaveSettings: {
            activity?: components["schemas"]["WaveActivity"];
            character?: components["schemas"]["WaveCharacter"];
            language?: components["schemas"]["WaveLanguage"];
            mood?: components["schemas"]["WaveMood"];
            seed?: components["schemas"]["WaveSeed"] | null;
            /**
             * Format: int32
             * @description Темп бега, шагов в минуту (140–200), для «Тренируюсь»: темп треков подстраивается под него.
             */
            tempo?: number | null;
        };
        /** @description Схемы нет в OpenAPI сервера — заглушка scripts/api-types.mjs */
        DeezerKind: string;
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    acquisition: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id скачивания (`acquisitionId` из ответа `/stream`) */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Шаг скачивания, место в очереди, есть ли кому качать */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Acquisition"];
                };
            };
            /** @description Нет такого скачивания */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list: {
        parameters: {
            query?: {
                /** @description Например `disputed` — раздел «Спорные». */
                status?: components["schemas"]["AcquisitionStatus"];
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Скачивания, недавние первыми: трек, кандидаты с разбором оценки, попытки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminAcquisitionPage"];
                };
            };
            /** @description Неверный cursor или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    resolve: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id скачивания */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ResolveRequest"];
            };
        };
        responses: {
            /** @description `choose`, `url`, `retry` — новое скачивание вне очереди; `unavailable` — это же скачивание, запись помечена «нет в источниках» */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminAcquisition"];
                };
            };
            /** @description Нет или неверный `videoId` / `url` */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого скачивания */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Скачивание записи ещё идёт (`acquisition_active`) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    audit_log: {
        parameters: {
            query?: {
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description Сколько событий вернуть: 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description События журнала, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuditPage"];
                };
            };
            /** @description Неверный cursor или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_downloaders: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Загрузчики: на связи первыми, отозванные в конце */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DownloaderList"];
                };
            };
            /** @description Нет права admin */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    create_downloader: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewDownloader"];
            };
        };
        responses: {
            /** @description Загрузчик заведён; токен `dwk_…` показывается один раз */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DownloaderToken"];
                };
            };
            /** @description Имя — от 1 до 60 символов */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    revoke_downloader: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id загрузчика */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Токен больше не действует; задания, взятые загрузчиком, вернулись в очередь */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого живого загрузчика */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_jobs: {
        parameters: {
            query?: {
                lane?: string;
                status?: components["schemas"]["JobStatus"];
                kind?: string;
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Задачи, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JobPage"];
                };
            };
            /** @description Неверный cursor или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    cancel_job: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id задачи */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Задача отменена; идущую исполнитель прервёт при следующем пульсе */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Job"];
                };
            };
            /** @description Нет такой задачи */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Задача уже закончилась (`job_finished`) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    retry_job: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id задачи */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Задача снова в очереди, попытки с нуля */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Job"];
                };
            };
            /** @description Нет такой задачи */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Задача ещё в работе (`job_active`) или такая же уже стоит в очереди (`job_duplicate`) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_lanes: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Полосы очереди со счётчиками */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LaneList"];
                };
            };
            /** @description Нет права admin */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    update_lane: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Имя полосы, например discography */
                lane: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["LanePatch"];
            };
        };
        responses: {
            /** @description Полоса после изменения */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Lane"];
                };
            };
            /** @description Параллельность 1–16, пауза 0–3 600 000 мс */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такой полосы */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    overview: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Размер библиотеки, записи по состоянию аудио, хранилище, очередь, сутки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminOverview"];
                };
            };
            /** @description Нет права admin */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    reco_overview: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Индекс и анализ, грани вкуса, последние показы, сервис анализа, настройки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecoOverview"];
                };
            };
            /** @description Нет права admin */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    run_reco: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RecoRun"];
            };
        };
        responses: {
            /** @description Задачи поставлены (уже стоящие в очереди не дублируются) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecoRunResult"];
                };
            };
            /** @description Неизвестная задача */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    update_reco_settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RecoSettingsPatch"];
            };
        };
        responses: {
            /** @description Настройки после изменения; разведчик применит их в следующий проход */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecoSettings"];
                };
            };
            /** @description Треков в сутки 0–50 000, предел индекса 100–2 000 000, шагов 0–3, затухание 0,05–0,95, часовой пояс −720…840 минут */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_sessions: {
        parameters: {
            query?: {
                /** @description Только сессии этого пользователя. */
                userId?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Живые сессии, свежие первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SessionList"];
                };
            };
            /** @description Нет права admin */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    revoke_session: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id сессии */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сессия отозвана; у приложения отозвано и устройство */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такой живой сессии */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    sources: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Загрузчики с работой за сутки и состояние серверного fetcher */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Sources"];
                };
            };
            /** @description Нет права admin */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    list_users: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Все люди; ждущие доступа — первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserList"];
                };
            };
            /** @description Нет права admin */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    update_user: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id пользователя */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UserPatch"];
            };
        };
        responses: {
            /** @description Пользователь после изменения */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminUser"];
                };
            };
            /** @description Нет такого пользователя */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нельзя снять admin с себя (`cannot_demote_self`) или заблокировать себя (`cannot_disable_self`) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    album: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id альбома */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Альбом и треки по порядку */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AlbumDetail"];
                };
            };
            /** @description Нет такого альбома */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    artist: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id артиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Карточка, популярное, дискография по типам, похожие, подписка */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtistDetail"];
                };
            };
            /** @description Нет такого артиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    app_start: {
        parameters: {
            query?: {
                /** @description `dalywave://auth`, `http://127.0.0.1:<port>/callback` или `http://localhost:<port>/callback`. */
                redirect_uri?: string;
                /** @description base64url(SHA-256(code_verifier)), 43–128 символов. */
                code_challenge?: string;
                /** @description Только `S256`. */
                code_challenge_method?: string;
                /** @description Возвращается приложению без изменений. */
                state?: string;
                /** @description Имя устройства для списка сессий, например «MacBook Захара». */
                device_name?: string;
                platform?: components["schemas"]["Platform"];
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description На daly.rest; после входа — на redirect_uri с `code` и `state` или с `error` и `state` */
            302: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description redirect_uri не из разрешённых */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    app_token: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AppTokenRequest"];
            };
        };
        responses: {
            /** @description Устройство и его токен; токен показывается один раз */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AppToken"];
                };
            };
            /** @description Код неверный, истёк, уже использован или не сходится с PKCE (`invalid_grant`) */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Аккаунт заблокирован */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    logout: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сессия отозвана, cookie `dw_session` стёрта */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    me: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Кто вошёл; у людей без ролей `roles` пустой */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Me"];
                };
            };
            /** @description Нет сессии */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    oidc_callback: {
        parameters: {
            query?: {
                code?: string;
                state?: string;
                /** @description Ошибка провайдера, например `access_denied`, если вход отменили */
                error?: string;
                /** @description RFC 9207: кто выдал ответ */
                iss?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Успех: cookie `dw_session` и переход на `next` (или код приложению). Ошибка: `/login?error=cancelled|expired|state|disabled|provider` */
            302: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    oidc_start: {
        parameters: {
            query?: {
                /** @description Куда вернуть после входа: путь на этом же сайте, например `/library`. */
                next?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description На страницу входа daly.rest; ставит cookie `dw_oidc` на 10 минут */
            302: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description `next` — не путь на этом сайте */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Вход через daly.rest не настроен */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    password_login: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PasswordLogin"];
            };
        };
        responses: {
            /** @description Вход как владелец; ставит cookie `dw_session` */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Me"];
                };
            };
            /** @description Неверный пароль (`wrong_password`) */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Вход паролем выключен (`password_disabled`) */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Больше 5 попыток в минуту с одного адреса */
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    providers: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Какие способы входа включены */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AuthProviders"];
                };
            };
        };
    };
    resolve_deezer: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description artist, album или track */
                kind: components["schemas"]["DeezerKind"];
                /** @description id у Deezer */
                deezerId: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Наша сущность; при первом обращении загружается из Deezer */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CatalogEntity"];
                };
            };
            /** @description В Deezer такого нет */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Deezer не ответил */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    ingest: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["EventBatch"];
            };
        };
        responses: {
            /** @description Сколько принято, сколько уже было, что отклонено и почему */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EventResult"];
                };
            };
            /** @description Больше 500 событий одного вида */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    claim: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ClaimRequest"];
            };
        };
        responses: {
            /** @description Задание: трек Deezer, подписанная ссылка для файла, срок аренды */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ClaimResponse"];
                };
            };
            /** @description Заданий нет: сервер подождал до 25 секунд. `Retry-After` — через сколько секунд спросить снова */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Токен загрузчика не принят */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    renew_lease: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id задания */
                id: string;
            };
            cookie?: never;
        };
        /** @description Можно не передавать */
        requestBody: {
            content: {
                "application/json": components["schemas"]["LeaseRequest"];
            };
        };
        responses: {
            /** @description Аренда продлена */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Lease"];
                };
            };
            /** @description Токен загрузчика не принят */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Задание уже не ваше (`job_not_yours`): аренда кончилась или его отменили — работу можно бросить */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    release: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id задания */
                id: string;
            };
            cookie?: never;
        };
        /** @description Можно не передавать */
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReleaseRequest"];
            };
        };
        responses: {
            /** @description Задание снова в очереди и его можно брать сразу; попытка не потрачена */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Токен загрузчика не принят */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого задания */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Задание уже не ваше (`job_not_yours`): аренда кончилась, его отменили или оно закончено */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    report: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id задания */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ResultReport"];
            };
        };
        responses: {
            /** @description Отчёт принят: файл записан, отдан в «Спорные» или задание вернулось в очередь */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Токен загрузчика не принят */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого задания */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Задание уже не ваше (`job_not_yours`): аренда кончилась или его отменили */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    healthz: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description API и база работают */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Health"];
                };
            };
            /** @description База недоступна */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Health"];
                };
            };
        };
    };
    home: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Полки главной; пустых нет */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Home"];
                };
            };
        };
    };
    image: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description albums или artists */
                kind: string;
                /** @description id альбома или артиста */
                id: string;
                /** @description 250.jpg, 500.jpg или 1000.jpg */
                file: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description JPEG; Cache-Control: public, max-age=31536000, immutable */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "image/jpeg": unknown;
                };
            };
            /** @description Копии ещё нет: картинка у Deezer, копия поставлена в очередь */
            302: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такой картинки */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    overview_get_library: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Счётчики, закреплённые плейлисты, подписки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LibraryOverview"];
                };
            };
        };
    };
    saved_albums: {
        parameters: {
            query?: {
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description Сколько вернуть: 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сохранённые альбомы, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SavedAlbumPage"];
                };
            };
            /** @description Неверный cursor или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    save_album: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id альбома */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Альбом в медиатеке (повтор безопасен), его треки поставлены на скачивание на сервер */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого альбома */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    unsave_album: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id альбома */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Альбом убран из медиатеки (если был) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    followed_artists: {
        parameters: {
            query?: {
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description Сколько вернуть: 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Подписки с политикой докачки и прогрессом, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["FollowedArtistPage"];
                };
            };
            /** @description Неверный cursor или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    follow: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id артиста */
                id: string;
            };
            cookie?: never;
        };
        /** @description Можно не передавать: политика по умолчанию */
        requestBody: {
            content: {
                "application/json": components["schemas"]["FollowRequest"];
            };
        };
        responses: {
            /** @description Подписка; новая или с другой политикой — ставит сверку дискографии */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["FollowedArtist"];
                };
            };
            /** @description Нет такого артиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    unfollow: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id артиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Подписки больше нет; скачанное остаётся на сервере */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    blocked: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description «Не рекомендовать»: треки и артисты, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["BlockedList"];
                };
            };
        };
    };
    block: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description track или artist */
                kind: components["schemas"]["BlockedKind"];
                /** @description id трека или артиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Больше не рекомендуется (повтор безопасен) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого трека или артиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    unblock: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description track или artist */
                kind: components["schemas"]["BlockedKind"];
                /** @description id трека или артиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Снова может рекомендоваться */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    liked_tracks: {
        parameters: {
            query?: {
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description Сколько вернуть: 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Понравившиеся треки, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LikedTrackPage"];
                };
            };
            /** @description Неверный cursor или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    like: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Трек в понравившихся (повтор безопасен) и поставлен на скачивание на сервер */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого трека */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    unlike: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Трек убран из понравившихся (если был) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    history: {
        parameters: {
            query?: {
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description Сколько вернуть: 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Прослушивания, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HistoryPage"];
                };
            };
            /** @description Неверный cursor или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    openapi: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Это описание API, OpenAPI 3.1 */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": unknown;
                };
            };
        };
    };
    list_get_playlists: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Плейлисты: закреплённые, затем недавно изменённые */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistList"];
                };
            };
        };
    };
    create: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewPlaylist"];
            };
        };
        responses: {
            /** @description Плейлист создан */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistDetail"];
                };
            };
            /** @description Пустое название или неизвестные треки */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    detail: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Плейлист и треки по порядку */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistDetail"];
                };
            };
            /** @description Нет такого плейлиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    remove: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Плейлист удалён вместе с позициями */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого плейлиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    update: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PlaylistPatch"];
            };
        };
        responses: {
            /** @description Плейлист после изменения */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Нет такого плейлиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    add_items: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AddPlaylistItems"];
            };
        };
        responses: {
            /** @description Добавленные позиции */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AddedPlaylistItems"];
                };
            };
            /** @description Неизвестные треки или позиция */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого плейлиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    remove_item: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
                /** @description id позиции */
                itemId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Позиция удалена */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого плейлиста или позиции */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    move_item: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
                /** @description id позиции */
                itemId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MovePlaylistItem"];
            };
        };
        responses: {
            /** @description Новая позиция */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MovedPlaylistItem"];
                };
            };
            /** @description Нет позиции `afterItemId` */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого плейлиста или позиции */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    pin: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Закреплён в медиатеке */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого плейлиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    unpin: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Больше не закреплён */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого плейлиста */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    search: {
        parameters: {
            query: {
                /** @description Строка поиска. */
                q: string;
                /** @description `catalog` (по умолчанию) или `library`. */
                scope?: components["schemas"]["SearchScope"];
                /** @description Через запятую: `track`, `album`, `artist`. По умолчанию все. */
                types?: string;
                /** @description Сколько результатов каждого вида: 1–50, по умолчанию 20. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Найденное: треки, альбомы, артисты */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SearchResults"];
                };
            };
            /** @description Пустой запрос, неверные types или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Deezer не ответил (только `scope=catalog`) */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    sound_search: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SoundSearch"];
            };
        };
        responses: {
            /** @description Ближайшее по звучанию к описанию: из медиатеки и из индекса каталога */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SoundSearchResults"];
                };
            };
            /** @description Пустой или слишком длинный запрос, limit вне 1–50 */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Сервис анализа не ответил */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    sync: {
        parameters: {
            query?: {
                /** @description `cursor` из предыдущего ответа; без него — полная синхронизация. */
                since?: string;
                /** @description Сколько изменений в странице: 1–2000, по умолчанию 500. Связанные строки каталога — сверх этого. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Изменения после курсора и следующий курсор */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SyncPage"];
                };
            };
            /** @description Неверный since или limit */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    lookup_tracks: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TrackLookup"];
            };
        };
        responses: {
            /** @description Треки в том же порядке; неизвестные пропущены */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackList"];
                };
            };
            /** @description Больше 500 id */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    track: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Трек со всеми участниками */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Track"];
                };
            };
            /** @description Нет такого трека */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    similar: {
        parameters: {
            query?: {
                /** @description 1–50, по умолчанию 20. */
                limit?: number;
            };
            header?: never;
            path: {
                /** @description id трека */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Похожие по звучанию, не больше двух у артиста. Трек ещё не разобран — популярное похожих артистов, а превью уже анализируется */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WaveItems"];
                };
            };
            /** @description limit вне 1–50 */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого трека */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    stream: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description `status: ready` — подписанная ссылка на файл (6 часов), формат, громкость, форма волны */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackStream"];
                };
            };
            /** @description `status: acquiring` — трек скачивается вне очереди; `waiting_for_downloader` — в очереди, но качать некому: загрузчик на Mac не на связи */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackStream"];
                };
            };
            /** @description Нет такого трека */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Запись ждёт решения в «Спорных» (`audio_disputed`) или её нет в источниках (`audio_unavailable`) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Deezer не ответил (запись трека ещё неизвестна) или хранилище недоступно */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    check: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Платформа: `darwin`, `windows`, `linux`, `ios` */
                target: string;
                /** @description Архитектура: `aarch64`, `x86_64` */
                arch: string;
                /** @description Установленная версия, например `0.1.0` */
                current_version: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Обновлений нет */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    create_session: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewWaveSession"];
            };
        };
        responses: {
            /** @description Сессия волны; с `seed` — радио от трека, артиста, альбома, плейлиста или описания */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WaveSession"];
                };
            };
            /** @description Неверные настройки: нет id или текста затравки, темп вне 100–220 */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет трека, артиста, альбома или плейлиста затравки */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Волна по описанию: сервис анализа не ответил */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    session: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id сессии */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сессия: настройки, подпись, сколько выдано */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WaveSession"];
                };
            };
            /** @description Нет такой сессии у этого человека */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    feedback: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id сессии */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["WaveFeedback"];
            };
        };
        responses: {
            /** @description Реакция учтена: следующая выдача её учитывает; лайк — ещё и в медиатеке */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такой сессии или трека */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    next: {
        parameters: {
            query?: {
                /** @description Сколько треков: 1–30, по умолчанию 10. */
                count?: number;
            };
            header?: never;
            path: {
                /** @description id сессии */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Следующие треки с причиной, темпом и тональностью; то, чего нет на сервере, уже скачивается (полоса «Волна»). Выданное в сессии не повторяется */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WaveItems"];
                };
            };
            /** @description count вне 1–30 */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такой сессии */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    ws: {
        parameters: {
            query?: {
                /** @description Токен сессии (`dwd_…` или `dws_…`) для клиентов без заголовков. */
                token?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description WebSocket: сообщения `ServerEvent` от сервера, `ClientMessage` от клиента */
            101: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Это не запрос WebSocket */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет сессии */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет ролей (`no_access`) или чужой Origin у cookie (`bad_origin`) */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
}
