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
    "/api/admin/acquisitions/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["one"];
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
    "/api/admin/imports": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["imports"];
        put?: never;
        post?: never;
        delete?: never;
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
    "/api/admin/recordings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["recordings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/recordings/{id}/reacquire": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["reacquire"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/rewind": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["event_settings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_event"];
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
    "/api/admin/settings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["settings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_settings"];
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
    "/api/admin/sources/check": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["check_download"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/storage": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["storage"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/admin/tracks/{id}/preview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["preview"];
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
    "/api/artists/{id}/about": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["artist_about"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/auth/app/pair": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["app_pair"];
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
    "/api/charts": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_charts"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/charts/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["chart"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/describe": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["describe_preview"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/describe/more": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["describe_more"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/describe/save": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["save_described"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["devices"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices/current/downloads": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_downloads"];
        put?: never;
        post: operations["add_download"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices/current/downloads/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["remove_download"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices/current/files": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["files"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices/current/set": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["rebuild_set"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices/remote": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["remote_presence"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices/remote/command": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["remote_command"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/devices/remote/transfer": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["remote_transfer"];
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
    "/api/friends": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_get_friends"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/invite": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["invite"];
        put?: never;
        post: operations["reset_invite"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/invite/{code}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["invite_preview"];
        put?: never;
        post: operations["use_invite"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/people": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["people"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/privacy": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["get_privacy"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_privacy"];
        trace?: never;
    };
    "/api/friends/requests": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["request"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/requests/{userId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["cancel_request"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/requests/{userId}/accept": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["accept"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/requests/{userId}/decline": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["decline"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/friends/{userId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["profile"];
        put?: never;
        post?: never;
        delete: operations["unfriend"];
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
    "/api/imports": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_get_imports"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/files": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["start_files"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/list": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["import_list"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/spotify": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["upload_spotify"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/yandex": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["import_yandex"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/youtube": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["import_youtube"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/youtube/takeout": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["upload_takeout"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["one_get_imports_id_"];
        put?: never;
        post?: never;
        delete: operations["remove"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/{id}/files": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["upload_file"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/{id}/finish": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["finish_files"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/{id}/items": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["items"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/imports/{id}/items/{itemId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["fix_item"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["jam_overview"];
        put?: never;
        post: operations["jam_start"];
        delete: operations["jam_end"];
        options?: never;
        head?: never;
        patch: operations["jam_update"];
        trace?: never;
    };
    "/api/jam/guest": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["jam_guest_view"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guest/leave": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["jam_leave"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guest/queue": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["jam_add"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guest/react": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["jam_react"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guest/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["jam_search"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guest/skip": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["jam_skip"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guest/stream/{trackId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["jam_stream"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guest/ws": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["jam_guest_ws"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/guests/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["jam_remove_guest"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/join/{code}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["jam_invite"];
        put?: never;
        post: operations["jam_join"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/requests/{id}/approve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["jam_approve"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/jam/requests/{id}/decline": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["jam_decline"];
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
    "/api/lyrics/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["search_lyrics"];
        put?: never;
        post?: never;
        delete?: never;
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
        delete: operations["clear_history"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/me/history/days": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["history_days"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/me/history/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["hide_play"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/notifications": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_get_notifications"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/notifications/push": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["push_info"];
        put?: never;
        post: operations["subscribe"];
        delete: operations["unsubscribe"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/notifications/read": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["read"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/notifications/settings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["get_settings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_settings_patch_notifications_settings"];
        trace?: never;
    };
    "/api/notifications/test": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["test"];
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
    "/api/overlay": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["info"];
        put?: never;
        post?: never;
        delete: operations["disable"];
        options?: never;
        head?: never;
        patch: operations["update"];
        trace?: never;
    };
    "/api/overlay/link": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["rotate"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/overlay/now": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["now"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/overlay/stream": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["stream_now"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlist-folders": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["create"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlist-folders/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["remove_delete_playlist_folders_id_"];
        options?: never;
        head?: never;
        patch: operations["update_patch_playlist_folders_id_"];
        trace?: never;
    };
    "/api/playlist-folders/{id}/move": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["move_folder"];
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
        post: operations["create_post_playlists"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/join/{code}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["join_preview"];
        put?: never;
        post: operations["join"];
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
        delete: operations["remove_delete_playlists_id_"];
        options?: never;
        head?: never;
        patch: operations["update_patch_playlists_id_"];
        trace?: never;
    };
    "/api/playlists/{id}/collaborative": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["make_collaborative"];
        post?: never;
        delete: operations["stop_collaborative"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/cover": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["set_cover"];
        post?: never;
        delete: operations["remove_cover"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/describe": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["described"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_described"];
        trace?: never;
    };
    "/api/playlists/{id}/describe/more": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["extend_described"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/duplicate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["duplicate"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/invite": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["invite_get_playlists_id_invite"];
        put?: never;
        post: operations["reset_invite_post_playlists_id_invite"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
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
    "/api/playlists/{id}/items/remove": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["remove_items"];
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
    "/api/playlists/{id}/members": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["add_members"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/members/{userId}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["remove_member"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/move": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["move_playlist"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/order": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["set_order"];
        post?: never;
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
    "/api/playlists/{id}/set": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["apply"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/set/preview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["preview_post_playlists_id_set_preview"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/playlists/{id}/visibility": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["set_visibility"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/recognize/history": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["recognize_history"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/recognize/history/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["recognize_delete"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/recognize/resolve": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["recognize_resolve"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/releases": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["register"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/releases/uploads": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["upload"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/resolve-link": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["resolve_link"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/rewind": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["years"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/rewind/{year}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["year"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/rewind/{year}/more": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["more"];
        put?: never;
        post: operations["save_more"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/rewind/{year}/playlist": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["playlist"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/rewind/{year}/seen": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["seen"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/rewind/{year}/soundtrack": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["soundtrack"];
        put?: never;
        post?: never;
        delete?: never;
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
    "/api/settings": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["get_settings_get_settings"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch: operations["update_patch_settings"];
        trace?: never;
    };
    "/api/share/{kind}/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["card"];
        put?: never;
        post?: never;
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
    "/api/taste": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["overview_get_taste"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/taste/dampers/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["remove_damper"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/taste/facets/{id}/less": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["less"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/taste/reset": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["reset"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/taste/training/claim": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["train_claim"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/taste/training/{id}/fail": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["train_fail"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/taste/training/{id}/result": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["train_result"];
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
    "/api/tracks/suggest": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["suggest"];
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
    "/api/tracks/{id}/analysis": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["track_analysis"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}/credits": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["track_credits"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}/hook": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["hook"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}/lyrics": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["track_lyrics"];
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
    "/api/tracks/{id}/versions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["track_versions"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}/versions/choice": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["choose"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/tracks/{id}/wrong-recording": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["report_wrong_recording"];
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
    "/api/versions/choices": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["choices"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/wave/presets": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["list_get_wave_presets"];
        put?: never;
        post: operations["create_post_wave_presets"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/wave/presets/order": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put: operations["reorder"];
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/wave/presets/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        delete: operations["remove_delete_wave_presets_id_"];
        options?: never;
        head?: never;
        patch: operations["update_patch_wave_presets_id_"];
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
    "/api/wave/why": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["why"];
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
            /**
             * @description Слушатель отметил основной файл как «Неверная запись»: новый файл заменяет его, даже если
             *     прежний «лучше» по качеству.
             */
            replace?: boolean;
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
        /** @description Годы на сцене. `to: null` — по сей день. */
        ActiveYears: {
            /** Format: int32 */
            from: number;
            /** Format: int32 */
            to: number | null;
        };
        /** @description Откуда играет: альбом, плейлист, артист — нажатие открывает его. */
        ActivityContext: {
            id: string;
            title: string;
            type: components["schemas"]["ContextType"];
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
        /** @description Пригласить друзей в совместный плейлист. */
        AddPlaylistMembers: {
            userIds: string[];
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
        AdminImport: {
            /** Format: date-time */
            createdAt: string;
            error: string | null;
            /** Format: date-time */
            finishedAt: string | null;
            /** Format: uuid */
            id: string;
            /** @description `text`, `csv`, `m3u`, `files`, `spotify_archive`. */
            kind: string;
            /** Format: int32 */
            matched: number;
            /** @description `uploading`, `matching`, `applying`, `done`. */
            phase: string;
            /**
             * Format: int32
             * @description Разобрано позиций из `total`.
             */
            processed: number;
            /** @description Что добавилось в медиатеку. */
            stats: components["schemas"]["ImportStats"];
            /** @description `running`, `done`, `failed`, `cancelled`. */
            status: string;
            title: string;
            /** Format: int32 */
            total: number;
            /**
             * Format: int64
             * @description Несопоставленных позиций.
             */
            unmatched: number;
            userName: string;
        };
        AdminImportList: {
            items: components["schemas"]["AdminImport"][];
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
        /** @description Запись в разделе «Каталог». */
        AdminRecording: {
            audioStatus: components["schemas"]["AudioStatus"];
            /** Format: int32 */
            durationMs: number;
            file: components["schemas"]["AudioFileInfo"] | null;
            /**
             * Format: int64
             * @description Всего файлов у записи, с вытесненными.
             */
            files: number;
            /** Format: uuid */
            id: string;
            isrc: string | null;
            /** Format: date-time */
            statusChangedAt: string;
            track: components["schemas"]["Track"] | null;
            /**
             * Format: int64
             * @description Сколько треков каталога делят запись.
             */
            tracks: number;
        };
        AdminRecordingPage: {
            items: components["schemas"]["AdminRecording"][];
            nextCursor: string | null;
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
        /** @description Предложенные точки сведения: с какой сильной доли входить в трек и где из него выходить. */
        AnalysisCues: {
            /** Format: int32 */
            mixInMs: number;
            /** Format: int32 */
            mixOutMs: number;
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
        /**
         * @description Среда APNs: отладочные сборки получают токены песочницы, TestFlight и App Store — боевые.
         * @enum {string}
         */
        ApnsEnvironment: "sandbox" | "production";
        AppRelease: {
            arch: string;
            notes?: string | null;
            /** Format: date-time */
            publishedAt: string;
            /** Format: int64 */
            sizeBytes: number;
            target: string;
            version: string;
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
        /** @description Коротко об артисте из Википедии. */
        ArtistAbout: {
            active: components["schemas"]["ActiveYears"] | null;
            /** @description До трёх жанров. */
            genres: string[];
            /** @description Группа, а не человек. */
            group: boolean;
            /** @description Язык статьи: `ru` или `en`. */
            lang: string;
            /** @description Город рождения или основания. */
            origin: string | null;
            /** @description Начало статьи: несколько предложений, без ударений. */
            summary: string;
            /** @description Статья целиком — «Читать дальше». */
            url: string;
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
        /** @description Ответ `GET /api/artists/{id}/about`. `about: null` — статьи нет, блока нет. */
        ArtistInfo: {
            about: components["schemas"]["ArtistAbout"] | null;
            /** Format: uuid */
            artistId: string;
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
        /** @description Аудиофайл записи с техническими данными. */
        AudioFileInfo: {
            /** @description `server` или `device:<id>`. */
            acquiredBy: string | null;
            /** Format: int32 */
            bitrateKbps: number | null;
            codec: string;
            container: string;
            /** Format: date-time */
            createdAt: string;
            /** Format: int32 */
            durationMs: number;
            /** Format: uuid */
            id: string;
            lossless: boolean;
            /** Format: float */
            loudnessLufs: number | null;
            /** Format: float */
            matchScore: number | null;
            /** Format: int32 */
            sampleRate: number | null;
            /** Format: int64 */
            sizeBytes: number;
            source: string;
            /** @description id видео или ссылка. */
            sourceRef: string | null;
            sourceTitle: string | null;
            /** Format: float */
            truePeakDb: number | null;
            verified: boolean;
            /** Format: float */
            verifySimilarity: number | null;
        };
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
        /** @description Артист-ориентир («как Скриптонит») или исключение («без Моргенштерна»). */
        BriefArtist: {
            /**
             * Format: uuid
             * @description Найденный артист каталога; `None` — не нашли.
             */
            id: string | null;
            /** @description Имя артиста в каталоге (или как написано, если не нашли). */
            name: string;
            /** @description Как написал человек: «Скриптонита». */
            said: string;
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
         * @description Качество по сотовой сети: «Обычное» или «Экономия» — Opus 64 кбит/с.
         * @enum {string}
         */
        CellularQuality: "normal" | "saver";
        /** @description Чарт целиком: треки по местам, альбомы и артисты — по трекам чарта. */
        ChartDetail: {
            /** @description Альбомы и EP, чьи треки стоят выше всего. */
            albums: components["schemas"]["AlbumSummary"][];
            /** @description Артисты, чьи треки стоят выше всего. */
            artists: components["schemas"]["ArtistSummary"][];
            chart: components["schemas"]["ChartSummary"];
            /** @description По местам: первый — №1. */
            tracks: components["schemas"]["Track"][];
            /** @description Чарт собирается прямо сейчас: стоит спросить ещё раз через несколько секунд. */
            updating: boolean;
        };
        /** @enum {string} */
        ChartKind: "country" | "world" | "genre";
        ChartList: {
            items: components["schemas"]["ChartSummary"][];
        };
        /** @description Чарт в списке. */
        ChartSummary: {
            /** @description `ru`, `world`, `genre-116`. */
            id: string;
            image: components["schemas"]["ImageSet"] | null;
            kind: components["schemas"]["ChartKind"];
            /** @description По-русски; интерфейс переводит по `id`. */
            title: string;
            /**
             * Format: date-time
             * @description Когда чарт собран. `null` — ещё ни разу.
             */
            updatedAt: string | null;
        };
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
        ClientMessage: (components["schemas"]["Ping"] & {
            /** @enum {string} */
            type: "ping";
        }) | (components["schemas"]["ConnectHello"] & {
            /** @enum {string} */
            type: "connect.hello";
        }) | (components["schemas"]["ConnectEmpty"] & {
            /** @enum {string} */
            type: "connect.heartbeat";
        }) | (components["schemas"]["ConnectPublish"] & {
            /** @enum {string} */
            type: "connect.state";
        }) | (components["schemas"]["ConnectSend"] & {
            /** @enum {string} */
            type: "connect.command";
        }) | (components["schemas"]["ConnectAck"] & {
            /** @enum {string} */
            type: "connect.ack";
        }) | (components["schemas"]["ConnectEmpty"] & {
            /** @enum {string} */
            type: "connect.claim";
        }) | (components["schemas"]["ConnectTransfer"] & {
            /** @enum {string} */
            type: "connect.transfer";
        }) | (components["schemas"]["ConnectSignal"] & {
            /** @enum {string} */
            type: "connect.signal";
        });
        /** @description Подтверждение команды: клиент шлёт его с `to` (кому), сервер доставляет с `from` (от кого). */
        ConnectAck: {
            /** @description Машинная причина: `offline` (устройства нет в сети), `failed`, `unsupported`. */
            error?: string | null;
            from?: string | null;
            id: string;
            ok: boolean;
            to?: string | null;
        };
        /**
         * @description Активным стало другое устройство (или то же после переподключения). Прежнее активное
         *     останавливает звук и становится пультом; при переносе `to` берёт `state` и продолжает.
         */
        ConnectActive: {
            deviceId: string | null;
            /** @description Кто был активным до этого. */
            from: string | null;
            state: components["schemas"]["PlaybackState"] | null;
            /** @description Перенос воспроизведения: новое активное устройство продолжает `state`. */
            transfer: boolean;
        };
        /** @description Что устройство умеет — пульт прячет то, чего нет. */
        ConnectCapabilities: {
            /** @description Режим переходов «Диджей» (движок приложения). */
            dj?: boolean;
            /** @description Только пульт: звук здесь не играет (часы), перенести сюда воспроизведение нельзя. */
            remote?: boolean;
            /** @description Громкость меняется из приложения (у Safari на iPhone — только кнопками телефона). */
            volume?: boolean;
        };
        /** @description Команда от устройства `from`: выполнить и ответить `connect.ack`. */
        ConnectCommand: {
            command: components["schemas"]["RemoteCommand"];
            from: string;
            id: string;
        };
        /** @description Устройство в списке «Где играть». */
        ConnectDevice: {
            /** @description Звук играет здесь (даже если устройство пропало из сети). */
            active: boolean;
            appVersion: string | null;
            capabilities: components["schemas"]["ConnectCapabilities"];
            id: string;
            kind: components["schemas"]["ConnectKind"];
            /** Format: date-time */
            lastSeenAt: string;
            name: string;
            /** @description Соединение открыто и устройство напоминало о себе за последние 15 с. */
            online: boolean;
            platform: components["schemas"]["Platform"];
        };
        /** @description Список устройств: активное — первым, затем в сети, затем недавние. */
        ConnectDevices: {
            activeDeviceId: string | null;
            devices: components["schemas"]["ConnectDevice"][];
        };
        /** @description Сигнал без данных: `connect.heartbeat`, `connect.claim`. */
        ConnectEmpty: {
            /** @description Необязательная метка — для журнала клиента; сервер её не читает. */
            id?: string | null;
        };
        /** @description `connect.handoff`: музыку подхватили (`transferred`) или предлагают подхватить на устройстве `to`. */
        ConnectHandoff: {
            from: string;
            /** @description Имя наушников («AirPods Захара»), если причина — они. */
            output: string | null;
            /** @description Продолжить со звуком (на прежнем устройстве музыка играла). */
            play: boolean;
            reason: components["schemas"]["HandoffReason"];
            to: string;
            /** @description Перенос уже сделан (режим «Сразу»); иначе — предложение «Продолжить здесь?». */
            transferred: boolean;
        };
        /** @description Первое сообщение устройства после `hello` сервера: кто оно. Повторяется после каждого переподключения. */
        ConnectHello: {
            appVersion?: string | null;
            capabilities?: components["schemas"]["ConnectCapabilities"];
            /** @description Постоянный id приложения или id вкладки; до 64 символов. */
            deviceId: string;
            kind: components["schemas"]["ConnectKind"];
            /** @description Имя для списка. У приложений сервер берёт имя устройства из входа («MacBook Pro Захара»). */
            name: string;
            platform: components["schemas"]["Platform"];
        };
        /**
         * @description Вид устройства — для значка в списке.
         * @enum {string}
         */
        ConnectKind: "web" | "laptop" | "desktop" | "phone" | "tablet" | "watch";
        /** @description Устройство вывода звука, о котором сообщает `connect.signal`. */
        ConnectOutput: {
            /** @description `builtin`, `headphones`, `bluetooth`, `airplay`, `usb`, `hdmi`, `other`. */
            kind: string;
            /** @description Имя, как его показывает система: «AirPods Захара». */
            name: string;
        };
        /** @description `GET /api/devices`: устройства и последнее состояние. */
        ConnectOverview: {
            activeDeviceId: string | null;
            devices: components["schemas"]["ConnectDevice"][];
            state: components["schemas"]["PlaybackState"] | null;
        };
        /** @description Активное устройство публикует снимок. */
        ConnectPublish: {
            state: components["schemas"]["PlaybackState"];
        };
        /** @description Команда устройству `to`. Ответ — `connect.ack` с тем же `id`. */
        ConnectSend: {
            command: components["schemas"]["RemoteCommand"];
            id: string;
            to: string;
        };
        /**
         * @description `connect.signal`: повод подхватить музыку на другом устройстве. Сервер сводит сигналы всех устройств
         *     человека и решает: перенести, предложить или ничего.
         */
        ConnectSignal: {
            output?: components["schemas"]["ConnectOutput"] | null;
            /** @description У `output_lost`: звук здесь играл, когда вывод пропал. */
            playing?: boolean;
            signal: components["schemas"]["ConnectSignalKind"];
        };
        /**
         * @description Что случилось с устройством.
         * @enum {string}
         */
        ConnectSignalKind: "output_lost" | "output_added" | "locked" | "unlocked" | "foreground";
        /** @description Снимок активного устройства — всем устройствам человека. */
        ConnectState: {
            deviceId: string;
            state: components["schemas"]["PlaybackState"];
        };
        /** @description «Перенести воспроизведение»: устройство `to` продолжит с того же места с той же очередью. */
        ConnectTransfer: {
            /** @description Если задан — ответ `connect.ack` с этим `id` (например, `offline`, если устройства нет в сети). */
            id?: string | null;
            /** @description Играть ли после переноса; не задано — как было. */
            play?: boolean | null;
            to: string;
        };
        /**
         * @description Откуда запущено.
         * @enum {string}
         */
        ContextType: "album" | "playlist" | "artist" | "wave" | "radio" | "mix" | "search" | "library" | "queue" | "remote";
        /** @description Человек и что он делал. Один человек — одна строка, роли по порядку важности. */
        CreditPerson: {
            /**
             * Format: uuid
             * @description Артист каталога, если MusicBrainz знает его страницу в Deezer: нажатие ведёт к его трекам.
             *     `null` — просто имя.
             */
            artistId: string | null;
            /**
             * Format: uuid
             * @description id в MusicBrainz.
             */
            id: string;
            name: string;
            roles: components["schemas"]["CreditRole"][];
        };
        /**
         * @description Роль в работе над треком (отношения MusicBrainz).
         * @enum {string}
         */
        CreditRole: "writer" | "composer" | "lyricist" | "arranger" | "producer" | "engineer" | "recording" | "mix" | "mastering" | "programming";
        DayPartShare: {
            /** @description `morning` (5–12), `day` (12–17), `evening` (17–23), `night`. */
            part: string;
            /**
             * Format: float
             * @description Доля времени прослушивания, 0..1.
             */
            share: number;
        };
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
        /**
         * @description Что понято из описания: по этому собирается плейлист. Отдаётся в предпросмотре и приходит обратно
         *     в «Ещё в том же духе» и при сохранении — повторно запрос не разбирается.
         */
        DescribeBrief: {
            /** @default any */
            activity: components["schemas"]["WaveActivity"];
            /**
             * @description Жанры, которых не надо («без попсы»).
             * @default []
             */
            avoidGenres: string[];
            /** @default mix */
            character: components["schemas"]["WaveCharacter"];
            /**
             * Format: int32
             * @description Число треков («20 треков»).
             * @default null
             */
            count: number | null;
            /**
             * @description В описании есть конкретика — жанр, ориентир или своё слово о звуке: подбор держится описания,
             *     а вкус только подмешивается. Без неё («40 минут для зала») — подбор из своего вкуса под настройки.
             * @default false
             */
            focused: boolean;
            /**
             * @description Жанры: `phonk`, `indie`, `rap` …
             * @default []
             */
            genres: string[];
            /** @default any */
            language: components["schemas"]["WaveLanguage"];
            /**
             * @description Ориентиры: «как Скриптонит».
             * @default []
             */
            like: components["schemas"]["BriefArtist"][];
            /**
             * Format: int32
             * @description Длительность, минут («40 минут», «на час»).
             * @default null
             */
            minutes: number | null;
            /** @default any */
            mood: components["schemas"]["WaveMood"];
            /**
             * @description «Без мата».
             * @default false
             */
            noExplicit: boolean;
            /**
             * @description «Без медляков».
             * @default false
             */
            noSlow: boolean;
            /**
             * @description Обстановка словами для названия: «для зала», «под дождь».
             * @default null
             */
            scene: string | null;
            /**
             * @description Текст для поиска по звучанию (CLAP): то, что описывает звук.
             * @default null
             */
            sound: string | null;
            /**
             * @description Описание как есть.
             * @default
             */
            text: string;
            /**
             * @description Исключённые артисты: «без Моргенштерна».
             * @default []
             */
            without: components["schemas"]["BriefArtist"][];
        };
        /** @description «Ещё в том же духе» для предпросмотра: следующие треки после `afterTrackId` без уже показанных. */
        DescribeMore: {
            /**
             * Format: uuid
             * @description Последний трек списка — к нему подстраивается начало новых.
             */
            afterTrackId?: string | null;
            brief: components["schemas"]["DescribeBrief"];
            /**
             * Format: int32
             * @description 1–30, по умолчанию 10.
             */
            count?: number | null;
            /** @description Уже показанные треки. */
            exclude?: string[];
        };
        /** @description Предпросмотр: название, описание и треки по порядку (плавно, как сет). */
        DescribePreview: {
            brief: components["schemas"]["DescribeBrief"];
            /** @description «Без медляков · только русское»; пустая строка — описывать нечего. */
            description: string;
            /**
             * Format: int64
             * @description Длительность всех треков.
             */
            durationMs: number;
            items: components["schemas"]["WaveItem"][];
            /** @description «Фонк для зала · 40 мин». */
            title: string;
        };
        /**
         * @description Собрать плейлист по описанию. `exclude` — треки прошлой сборки («Пересобрать»): их сервер
         *     постарается не повторять.
         */
        DescribeRequest: {
            exclude?: string[];
            /** @description 2–300 символов. */
            text: string;
        };
        /** @description Добавленное «Ещё в том же духе»: id треков по порядку. */
        DescribedAdded: {
            trackIds: string[];
        };
        /** @description «Ещё в том же духе» для сохранённого плейлиста: сколько добавить в конец. */
        DescribedMore: {
            /**
             * Format: int32
             * @description 1–30, по умолчанию 10.
             */
            count?: number | null;
        };
        /** @description Настройки сохранённого плейлиста по описанию. */
        DescribedPatch: {
            weekly: boolean;
        };
        /** @description Плейлист, собранный по описанию. */
        DescribedPlaylist: {
            brief: components["schemas"]["DescribeBrief"];
            /** Format: uuid */
            playlistId: string;
            /**
             * Format: date-time
             * @description Когда неделя обновила его в последний раз.
             */
            refreshedAt: string | null;
            weekly: boolean;
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
        /** @description Закреплённое на устройстве. */
        DeviceDownload: {
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["DownloadKind"];
            /** Format: uuid */
            refId: string | null;
            /**
             * Format: int64
             * @description Треков в нём на сервере сейчас и всего.
             */
            tracksReady: number;
            /** Format: int64 */
            tracksTotal: number;
        };
        DeviceDownloadList: {
            items: components["schemas"]["DeviceDownload"][];
        };
        /** @description Набор волны устройства после пересборки. Сами строки приходят синхронизацией (`deviceSets`). */
        DeviceSet: {
            /** Format: date-time */
            builtAt: string;
            /** Format: int64 */
            favorites: number;
            /** Format: int64 */
            fresh: number;
            /**
             * Format: int64
             * @description Ушло из набора: новое, которое уже сыграно, и то, что больше не подходит.
             */
            removed: number;
        };
        /** @description Собрать набор волны для устройства: сколько треков любимого и нового поместится в запас. */
        DeviceSetRequest: {
            /**
             * Format: int32
             * @description 0–2000.
             */
            favorites: number;
            /**
             * Format: int32
             * @description 0–2000.
             */
            fresh: number;
        };
        /**
         * @description Что закрепить на устройстве.
         * @enum {string}
         */
        DownloadKind: "track" | "album" | "playlist" | "artist" | "liked";
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
        /**
         * @description Качество загрузок на устройство.
         * @enum {string}
         */
        DownloadQuality: "high" | "normal";
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
        /** @description Копия плейлиста или микса — обычный плейлист человека. */
        DuplicatePlaylist: {
            /** @description Название копии; без него — как у исходного. */
            title?: string | null;
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
        /** @description Файл трека для загрузки на устройство. */
        FileLink: {
            codec: components["schemas"]["AudioCodec"];
            container: components["schemas"]["AudioContainer"];
            /** Format: int32 */
            durationMs: number;
            /** Format: date-time */
            expiresAt: string;
            /** Format: uuid */
            fileId: string;
            /** Format: float */
            loudnessLufs: number | null;
            mimeType: string;
            /** Format: uuid */
            recordingId: string;
            /** @description SHA-256 в hex, если известен. */
            sha256: string | null;
            /**
             * Format: int64
             * @description Размер файла: по нему приложение проверяет скачанное.
             */
            sizeBytes: number;
            /** Format: uuid */
            trackId: string;
            /** Format: float */
            truePeakDb: number | null;
            /** @description Подписанная ссылка на файл; поддерживает `Range` — недокачанное продолжается. */
            url: string;
            /** @description Огибающая для полосы прокрутки: 512 точек, 0..1000. */
            waveform: number[] | null;
        };
        FileLinks: {
            items: components["schemas"]["FileLink"][];
            /** @description Файла на сервере ещё нет: скачивание поставлено в очередь, спросить позже. */
            pending: string[];
        };
        /** @description Ссылки на файлы треков: до 200 за раз. */
        FileLinksRequest: {
            trackIds: string[];
        };
        /** @description Новый импорт файлов: дальше файлы грузятся по одному, затем `finish`. */
        FilesImportRequest: {
            title?: string | null;
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
        Friend: {
            activity: components["schemas"]["FriendActivity"] | null;
            person: components["schemas"]["PersonRef"];
            /** Format: date-time */
            since: string;
        };
        /** @description Что слушает (или слушал последним) друг. */
        FriendActivity: {
            /**
             * Format: date-time
             * @description Сейчас — когда начался трек; последнее — когда слушал.
             */
            at: string;
            context: components["schemas"]["ActivityContext"] | null;
            /** @description У друга открыто «Слушать вместе»: код для `/jam/{code}`. */
            jamCode: string | null;
            /** @description Играет прямо сейчас; иначе — последнее, что слушал, в `at`. */
            now: boolean;
            track: components["schemas"]["Track"];
        };
        /** @description Друг начал слушать другое (или перестал). */
        FriendActivityEvent: {
            activity: components["schemas"]["FriendActivity"] | null;
            /** Format: uuid */
            userId: string;
        };
        /** @description Друзья (слушающие сейчас — первыми) и запросы. */
        FriendList: {
            friends: components["schemas"]["Friend"][];
            /** @description Ждут моего ответа. */
            incoming: components["schemas"]["FriendRequest"][];
            /** @description Я отправил, ответа нет. */
            outgoing: components["schemas"]["FriendRequest"][];
        };
        /** @description Страница друга. */
        FriendProfile: {
            activity: components["schemas"]["FriendActivity"] | null;
            /** @description Ваши общие артисты: кого слушаете оба, самые общие первыми. */
            commonArtists: components["schemas"]["ArtistSummary"][];
            person: components["schemas"]["PersonRef"];
            /** @description Его плейлисты, открытые друзьям. */
            playlists: components["schemas"]["PlaylistSummary"][];
            relation: components["schemas"]["Relation"];
            /**
             * Format: date-time
             * @description Друзья с этого момента (только у друзей).
             */
            since: string | null;
            tasteMatch: components["schemas"]["TasteMatch"] | null;
            /** @description Топ артистов за 30 дней. */
            topArtists: components["schemas"]["FriendTopArtist"][];
        };
        FriendRequest: {
            /** Format: date-time */
            createdAt: string;
            person: components["schemas"]["PersonRef"];
        };
        /** @description Чем закончился запрос: отправлен или (встречный уже был) — сразу друзья. */
        FriendRequestResult: {
            relation: components["schemas"]["Relation"];
        };
        /** @description Артист в топе друга за месяц. */
        FriendTopArtist: {
            artist: components["schemas"]["ArtistSummary"];
            /** Format: int64 */
            plays: number;
        };
        /** @description Друзья или запросы изменились: перечитать список. */
        FriendsChange: {
            /** @description Что поменялось: `requests`, `friends`, `privacy`. */
            what: string;
        };
        /**
         * @description «Подхват на другом устройстве»: наушники перешли с компьютера на телефон (или телефон открыли, пока
         *     компьютер заблокирован) — музыка за ними. По умолчанию — предложение: ложный перенос (наушники просто
         *     сели) останавливает музыку там, где её слушали, а предложение стоит одного нажатия.
         * @enum {string}
         */
        HandoffMode: "off" | "suggest" | "auto";
        /**
         * @description Почему музыка переходит.
         * @enum {string}
         */
        HandoffReason: "headphones" | "away";
        /** @description Голова сети вкуса на отложенных по времени данных: у сети и у правил. */
        HeadMetrics: {
            /** Format: double */
            auc: number | null;
            /** Format: double */
            baselineAuc: number | null;
            /** Format: double */
            baselineLogloss: number | null;
            /** @description `skip`, `complete`, `like`. */
            head: string;
            /** Format: double */
            logloss: number | null;
            /**
             * Format: int64
             * @description Сколько положительных исходов в отложенной части.
             */
            positives: number;
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
        HistoryDay: {
            /** Format: date */
            day: string;
            /** Format: int64 */
            plays: number;
        };
        /** @description Дни с прослушиваниями, новые первыми. */
        HistoryDays: {
            days: components["schemas"]["HistoryDay"][];
        };
        HistoryItem: {
            contextId: string | null;
            /** @description Название источника: альбом, плейлист, артист; у волны, радио и поиска — нет. */
            contextTitle: string | null;
            contextType: components["schemas"]["ContextType"] | null;
            endReason: components["schemas"]["EndReason"];
            /** Format: uuid */
            id: string;
            /** @description Из истории Spotify (импорт архива). */
            imported?: boolean;
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
        /**
         * @description Откуда место отрывка.
         * @enum {string}
         */
        HookFrom: "analysis" | "estimate" | "preview";
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
        /** @description Кандидат Deezer для позиции, которая нашлась неуверенно. */
        ImportCandidate: {
            album: string | null;
            artist: string;
            /** Format: int64 */
            deezerId: number;
            /** Format: int32 */
            durationMs: number | null;
            /**
             * Format: float
             * @description Оценка совпадения 0..1.
             */
            score: number;
            title: string;
        };
        /** @description Загруженный файл: формат и что с ним стало. */
        ImportFileInfo: {
            /**
             * Format: int32
             * @default null
             */
            bitDepth: number | null;
            /**
             * Format: int32
             * @default null
             */
            bitrateKbps: number | null;
            /** @default null */
            codec: string | null;
            /**
             * @description Файл не прочитался: почему.
             * @default null
             */
            error: string | null;
            /** @default false */
            lossless: boolean;
            /** @default  */
            name: string;
            /**
             * Format: int32
             * @default null
             */
            sampleRate: number | null;
            /**
             * Format: int64
             * @default 0
             */
            sizeBytes: number;
            /**
             * @description Стал основным файлом записи (лучше прежнего).
             * @default false
             */
            upgraded: boolean;
            /**
             * @description Отпечаток совпал с превью Deezer; `null` — сравнить было не с чем.
             * @default null
             */
            verified: boolean | null;
        };
        ImportFilePayload: {
            /** Format: uuid */
            itemId: string;
            /**
             * Format: uuid
             * @description Трек выбрал человек: файл кладётся к нему без поиска.
             */
            trackId?: string | null;
        };
        ImportItem: {
            album: string | null;
            /** @description Как позиция называлась в источнике. */
            artist: string;
            /** @description Лучшие кандидаты, если найти уверенно не вышло. */
            candidates: components["schemas"]["ImportCandidate"][];
            /** Format: float */
            confidence: number | null;
            file: components["schemas"]["ImportFileInfo"] | null;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["ImportItemKind"];
            liked: boolean;
            /**
             * Format: int32
             * @description Сколько раз позиция встречается в истории прослушиваний.
             */
            plays: number;
            /** Format: int32 */
            position: number;
            status: components["schemas"]["ImportItemStatus"];
            title: string;
            track: components["schemas"]["Track"] | null;
        };
        /** @description Исправить позицию: выбрать трек (наш id из поиска или кандидат Deezer) или пропустить. */
        ImportItemFix: {
            /** Format: uuid */
            albumId?: string | null;
            /** Format: uuid */
            artistId?: string | null;
            /** Format: int64 */
            deezerId?: number | null;
            /** @description Не искать больше: позиция пропускается. */
            skip?: boolean;
            /** Format: uuid */
            trackId?: string | null;
        };
        /** @enum {string} */
        ImportItemKind: "track" | "album" | "artist" | "file";
        ImportItemPage: {
            items: components["schemas"]["ImportItem"][];
            nextCursor: string | null;
        };
        /** @enum {string} */
        ImportItemStatus: "pending" | "matched" | "unmatched" | "skipped";
        /** @description Задачи импорта (полоса «Импорт»). */
        ImportJobPayload: {
            /** Format: uuid */
            importId: string;
        };
        /**
         * @description Что импортируется. Хранится в `imports.kind`.
         * @enum {string}
         */
        ImportKind: "text" | "csv" | "m3u" | "files" | "spotify_archive" | "yandex" | "youtube_playlist" | "youtube_takeout";
        ImportList: {
            items: components["schemas"]["ImportSummary"][];
        };
        /**
         * @description Шаг импорта: файлы ещё грузятся → сопоставление с каталогом → раскладка в медиатеку → готово.
         * @enum {string}
         */
        ImportPhase: "uploading" | "matching" | "applying" | "done";
        /** @description Что добавилось в медиатеку. */
        ImportStats: {
            /**
             * Format: int64
             * @default 0
             */
            albums: number;
            /**
             * Format: int64
             * @default 0
             */
            artists: number;
            /**
             * Format: int64
             * @description Принятых файлов.
             * @default 0
             */
            files: number;
            /**
             * Format: int64
             * @default 0
             */
            likes: number;
            /**
             * Format: int64
             * @default 0
             */
            playlistTracks: number;
            /**
             * Format: int64
             * @default 0
             */
            playlists: number;
            /**
             * Format: int64
             * @description Прослушиваний из истории.
             * @default 0
             */
            plays: number;
            /**
             * Format: int64
             * @description Файлов, заменивших копию хуже качеством.
             * @default 0
             */
            upgraded: number;
        };
        /** @enum {string} */
        ImportStatus: "running" | "done" | "failed" | "cancelled";
        ImportSummary: {
            /** Format: date-time */
            createdAt: string;
            error: string | null;
            /** Format: date-time */
            finishedAt: string | null;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["ImportKind"];
            /** Format: int32 */
            matched: number;
            phase: components["schemas"]["ImportPhase"];
            /**
             * Format: uuid
             * @description Плейлист, куда легли треки списка.
             */
            playlistId: string | null;
            /**
             * Format: int32
             * @description Разобрано (найдено, не найдено или пропущено).
             */
            processed: number;
            stats: components["schemas"]["ImportStats"];
            status: components["schemas"]["ImportStatus"];
            title: string;
            /**
             * Format: int32
             * @description Позиций для сопоставления.
             */
            total: number;
            /**
             * Format: int64
             * @description Ждут проверки.
             */
            unmatched: number;
        };
        /**
         * @description Куда кладётся найденное из списка.
         * @enum {string}
         */
        ImportTarget: "likes" | "playlist" | "library";
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
        /** @description Ссылка «Добавить в друзья»: кто откроет её, станет другом сразу. */
        InviteLink: {
            code: string;
            /** @description Полный адрес для QR-кода и «Поделиться». */
            url: string;
        };
        /** @description Сессия глазами хозяина. */
        Jam: {
            code: string;
            /** Format: date-time */
            createdAt: string;
            /**
             * Format: date-time
             * @description Закроется сама, если до этого времени ничего не произойдёт.
             */
            expiresAt: string;
            guests: components["schemas"]["JamGuest"][];
            /** Format: uuid */
            id: string;
            /** @description Ждущие одобрения и добавленные (для подписи «добавил Миша» в очереди), новые первыми. */
            requests: components["schemas"]["JamRequest"][];
            settings: components["schemas"]["JamSettings"];
            /** @description Ссылка для гостей (и QR-кода). */
            url: string;
        };
        /** @description `POST /api/jam/guest/queue`. */
        JamAdd: {
            /** Format: uuid */
            trackId: string;
        };
        /**
         * @description `jam.changed`: участники, настройки или треки гостей изменились — перечитать. `ended` — сессия
         *     закрыта; `removed` — этого гостя убрал хозяин.
         */
        JamChanged: {
            ended?: boolean;
            /** Format: uuid */
            jamId: string;
            /** Format: uuid */
            removed: string | null;
        };
        /** @description Участник сессии. */
        JamGuest: {
            /** Format: uuid */
            id: string;
            /** Format: date-time */
            joinedAt: string;
            /** @description Вошёл со своим аккаунтом DalyWave. */
            member: boolean;
            name: string;
            /** @description Страница гостя открыта (был на связи последние 2 минуты). */
            online: boolean;
        };
        /** @description `GET /api/jam/guest`: всё для страницы гостя. */
        JamGuestView: {
            /** Format: date-time */
            expiresAt: string;
            guests: components["schemas"]["JamGuest"][];
            hostName: string;
            /** Format: uuid */
            jamId: string;
            me: components["schemas"]["JamGuest"];
            /** @description Свои треки (ждут одобрения, добавлены, отклонены), новые первыми. */
            mine: components["schemas"]["JamRequest"][];
            now: components["schemas"]["JamNow"] | null;
            settings: components["schemas"]["JamSettings"];
            /** @description Сколько голосов «пропустить» уже за текущий трек и сколько нужно. */
            skip: components["schemas"]["JamSkip"];
        };
        /** @description `GET /api/jam/join/{code}`: что показать на странице входа гостя. */
        JamInvite: {
            current: components["schemas"]["JamTrack"] | null;
            /** Format: int32 */
            guests: number;
            hostAvatar: string | null;
            hostName: string;
            /** @description Ссылку открыл человек со своим аккаунтом: имя возьмётся оттуда. */
            memberName: string | null;
        };
        /** @description `POST /api/jam/join/{code}`. */
        JamJoin: {
            /** @description Имя гостя, 1–32 символа. Со своим аккаунтом можно не указывать. */
            name?: string | null;
        };
        /** @description Гость вошёл. Токен показывается один раз: на сервере только его хеш. */
        JamJoined: {
            guest: components["schemas"]["JamGuest"];
            /** @description `dwj_…` для `Authorization: Bearer` на `/api/jam/guest/*`. */
            token: string;
        };
        /** @description Что играет у хозяина: трек, позиция и ближайшая очередь — без остального его состояния. */
        JamNow: {
            /**
             * Format: date-time
             * @description Время сервера, к которому относится позиция.
             */
            at: string;
            current: components["schemas"]["JamQueueItem"] | null;
            /** Format: int32 */
            durationMs: number;
            /** @description Активное устройство хозяина на связи. */
            live: boolean;
            playing: boolean;
            /**
             * Format: int32
             * @description Позиция на момент `at`.
             */
            positionMs: number;
            /** @description Ближайшие треки: добавленные вручную, затем остальное (до 30). */
            queue: components["schemas"]["JamQueueItem"][];
        };
        /** @description `jam.now` — гостям: что играет у хозяина (при каждом снимке его активного устройства). */
        JamNowEvent: {
            /** Format: uuid */
            jamId: string;
            now: components["schemas"]["JamNow"];
        };
        /** @description `GET /api/jam`: открытая сессия хозяина или `null`. */
        JamOverview: {
            jam: components["schemas"]["Jam"] | null;
        };
        /** @description `PATCH /api/jam`. */
        JamPatch: {
            approval?: boolean | null;
            guestsCanAdd?: boolean | null;
            guestsCanListen?: boolean | null;
        };
        /** @description Элемент очереди для гостя. */
        JamQueueItem: {
            /** @description Имя гостя, который добавил трек. */
            addedBy: string | null;
            track: components["schemas"]["JamTrack"];
        };
        /** @description `POST /api/jam/guest/react`. */
        JamReact: {
            kind: components["schemas"]["JamReactionKind"];
        };
        /** @description `jam.reaction` — всем в сессии: кто-то поставил ❤ текущему треку. */
        JamReaction: {
            guestName: string;
            /** Format: uuid */
            jamId: string;
            kind: components["schemas"]["JamReactionKind"];
            /** Format: uuid */
            trackId: string | null;
        };
        /** @enum {string} */
        JamReactionKind: "heart";
        /** @description Трек от гостя. */
        JamRequest: {
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            guestId: string;
            guestName: string;
            /** Format: uuid */
            id: string;
            status: components["schemas"]["JamRequestStatus"];
            track: components["schemas"]["JamTrack"];
        };
        /** @enum {string} */
        JamRequestStatus: "pending" | "added" | "declined";
        JamSearchResults: {
            items: components["schemas"]["JamTrack"][];
        };
        /** @description Настройки сессии. */
        JamSettings: {
            /** @description Добавленное гостями ждёт одобрения хозяина. */
            approval: boolean;
            /** @description «Гости могут добавлять». */
            guestsCanAdd: boolean;
            /** @description Гости могут слушать у себя. */
            guestsCanListen: boolean;
        };
        JamSkip: {
            /** Format: int32 */
            needed: number;
            /** @description Этот гость уже голосовал за текущий трек. */
            voted: boolean;
            /** Format: int32 */
            votes: number;
        };
        /** @description Ссылка на аудио для «Слушать у себя»: только текущий и следующий трек хозяина. */
        JamStream: {
            /** Format: int32 */
            durationMs: number;
            /** Format: date-time */
            expiresAt: string;
            /** Format: float */
            loudnessLufs: number | null;
            mimeType: string;
            /** Format: uuid */
            trackId: string;
            /** Format: float */
            truePeakDb: number | null;
            url: string;
        };
        /** @description Трек для гостя: только то, что видно на экране (без медиатеки хозяина). */
        JamTrack: {
            album: string | null;
            artists: string[];
            /** Format: int32 */
            durationMs: number;
            explicit: boolean;
            /** Format: uuid */
            id: string;
            image: components["schemas"]["ImageSet"] | null;
            title: string;
            /** @description «Slowed», «Remastered 2011»; пустая строка, если версии нет. */
            version: string;
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
            /**
             * Format: int32
             * @description Окно работы в минутах суток (часовой пояс владельца); `null` в обоих — круглосуточно.
             */
            activeFrom: number | null;
            /** Format: int32 */
            activeUntil: number | null;
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
            /** @description Сейчас внутри окна: вне его полоса задачи не выдаёт, как на паузе. */
            open: boolean;
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
            schedule?: components["schemas"]["LaneSchedule"] | null;
        };
        /**
         * @description Окно работы полосы: минуты суток 0–1439 по часовому поясу владельца, может переходить через полночь.
         *     Оба `null` — круглосуточно.
         */
        LaneSchedule: {
            /** Format: int32 */
            from?: number | null;
            /** Format: int32 */
            until?: number | null;
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
        /**
         * @description Откуда ссылка.
         * @enum {string}
         */
        LinkSource: "dalywave" | "deezer" | "spotify" | "apple" | "youtube" | "soundcloud" | "bandcamp";
        /** @enum {string} */
        ListFormat: "text" | "csv" | "m3u";
        /** @description Импорт списка. Формат без указания определяется по тексту. */
        ListImportRequest: {
            format?: components["schemas"]["ListFormat"] | null;
            /** @description Название нового плейлиста; по умолчанию — имя файла или «Импорт от …». */
            playlistTitle?: string | null;
            target?: components["schemas"]["ImportTarget"] | null;
            text: string;
        };
        /** @description Прослушивания за текущий месяц по времени слушателя. */
        ListeningStats: {
            /** Format: int64 */
            artists: number;
            dayParts: components["schemas"]["DayPartShare"][];
            /** Format: int64 */
            minutes: number;
            /** Format: int64 */
            plays: number;
            /**
             * Format: int64
             * @description Столько же за прошлый месяц — для сравнения.
             */
            previousMinutes: number;
            /**
             * Format: date-time
             * @description Начало месяца.
             */
            since: string;
            topArtists: components["schemas"]["TopArtist"][];
            topGenres: components["schemas"]["TopGenre"][];
            /** Format: int64 */
            tracks: number;
        };
        /** @description Трек, в тексте которого нашлась строчка. */
        LyricsHit: {
            /** @description Строка текста, где нашлось. */
            line: string;
            track: components["schemas"]["Track"];
        };
        /**
         * @description Что известно о тексте трека.
         * @enum {string}
         */
        LyricsKind: "synced" | "plain" | "instrumental" | "none";
        /** @description Строка синхронного текста. Пустой `text` — проигрыш без слов. */
        LyricsLine: {
            /**
             * Format: int32
             * @description Начало строки от начала трека, мс.
             */
            tMs: number;
            text: string;
        };
        LyricsPayload: {
            /** Format: uuid */
            trackId: string;
        };
        LyricsSearchResults: {
            items: components["schemas"]["LyricsHit"][];
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
        /** @description Собранный плейлист: «Микс дня» или «Открытия недели». */
        MixView: {
            /** @description `daily_mix` или `discover_weekly`. */
            generator: string;
            /** Format: uuid */
            id: string;
            title: string;
            /** Format: int64 */
            tracks: number;
            /** Format: date-time */
            updatedAt: string;
        };
        /** @description Сервис анализа звука. */
        MlStatus: {
            error: string | null;
            /** @description Ответ `/v1/healthz`: загружены ли модели, занят ли сервис, версии. */
            health: Record<string, never> | null;
            reachable: boolean;
            url: string;
        };
        /** @description Последнее обучение сети вкуса (задача `taste_model`). */
        ModelTraining: {
            /** @description Последние задания компьютерам, новые первыми. */
            deviceTasks: components["schemas"]["TrainTaskView"][];
            /** Format: date-time */
            lastRunAt: string | null;
            /** @description Что вышло: «Мало данных: 120 прослушиваний из 300», текст ошибки. */
            message: string | null;
            /** Format: int64 */
            minRows: number;
            /** @description `trained`, `too_little_data`, `failed`. */
            outcome: string | null;
            /**
             * Format: int64
             * @description Размеченных прослушиваний для обучения сейчас и сколько нужно.
             */
            rows: number;
            /** @description Обучение стоит в очереди или идёт. */
            running: boolean;
        };
        /** @description Версия модели вкуса: правила (`rules-1`) или обученная сеть (`nn-…`). */
        ModelVersionView: {
            /** @description Собирает поток сейчас. */
            active: boolean;
            /**
             * Format: double
             * @description Средние по головам AUC и logloss на отложенной части: у сети и у правил. У правил — `null`.
             */
            auc: number | null;
            /** Format: double */
            baselineAuc: number | null;
            /** Format: double */
            baselineLogloss: number | null;
            /** Format: double */
            baselineNdcg: number | null;
            /** Format: double */
            baselineSessionAuc: number | null;
            beatsCurrent: boolean | null;
            /** @description Точнее ли правил на отложенных данных — то есть включена ли при обучении. */
            beatsRules: boolean | null;
            /** Format: date-time */
            createdAt: string;
            /** Format: double */
            currentAuc: number | null;
            /** @description Версия, которая была включена при проверке, и её AUC на той же отложенной части. */
            currentVersion: string | null;
            /** @description Компьютер, на котором обучена. */
            deviceName: string | null;
            /**
             * Format: int64
             * @description Лучшая эпоха.
             */
            epochs: number | null;
            heads: components["schemas"]["HeadMetrics"][];
            /** Format: int64 */
            holdoutRows: number | null;
            /** Format: uuid */
            id: string;
            /** @description Сеть (`true`) или правила. */
            learned: boolean;
            /** Format: double */
            logloss: number | null;
            /** Format: double */
            ndcg: number | null;
            /** @description Почему включена или не включена: «точнее правил и текущей», «правила не хуже». */
            note: string | null;
            /** @description Где обучена: `server`, `device` (компьютер с приложением) или `rules`. */
            origin: string;
            /**
             * Format: double
             * @description Доля бутстреп-выборок по сессиям, где сеть точнее правил.
             */
            pBetter: number | null;
            /**
             * Format: int64
             * @description Обучаемых параметров.
             */
            params: number | null;
            /**
             * Format: double
             * @description Порядок внутри сессии на отложенной части: доля верных пар «пропустил — дослушал» и NDCG@10,
             *     у сети и у правил.
             */
            sessionAuc: number | null;
            /**
             * Format: int64
             * @description Строк на обучение и на проверку.
             */
            trainRows: number | null;
            /** @description Своя сеть слушателя; `null` — общая. */
            userName: string | null;
            version: string;
        };
        /**
         * @description Переложить плейлист: в папку (`folderId`) или из неё (`null`) и на место среди соседей. Сосед
         *     `afterId` — плейлист или (вне папок) папка; без него — в конец, при `atStart` — в начало.
         */
        MovePlaylist: {
            /** Format: uuid */
            afterId?: string | null;
            atStart?: boolean;
            /** Format: uuid */
            folderId?: string | null;
        };
        /** @description Переставить папку среди папок и плейлистов вне папок. */
        MovePlaylistFolder: {
            /** Format: uuid */
            afterId?: string | null;
            atStart?: boolean;
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
        /**
         * @description Сеть, по которой играет устройство: качество тогда берётся из настроек слушателя.
         * @enum {string}
         */
        NetworkKind: "wifi" | "cellular";
        /** @description Закрепить на этом устройстве. `refId` — у всех, кроме `liked`. */
        NewDeviceDownload: {
            kind: components["schemas"]["DownloadKind"];
            /** Format: uuid */
            refId?: string | null;
        };
        NewDownloader: {
            /** @description Например «Mac Захара». */
            name: string;
        };
        NewFriendRequest: {
            /** Format: uuid */
            userId: string;
        };
        NewPlaylist: {
            description?: string | null;
            title: string;
            /** @description Треки в этом порядке. */
            trackIds?: string[];
        };
        /** @description Новая папка; названные плейлисты сразу переезжают в неё. */
        NewPlaylistFolder: {
            playlistIds?: string[];
            title: string;
        };
        /** @description Новое сочетание — в конец ряда. */
        NewWavePreset: {
            icon: string;
            /** @description 1–40 символов. */
            name: string;
            settings: components["schemas"]["WaveSettings"];
        };
        NewWaveSession: {
            settings?: components["schemas"]["WaveSettings"];
        };
        Notification: {
            body: string;
            /**
             * Format: int32
             * @description Сколько событий собрано в уведомлении.
             */
            count: number;
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            id: string;
            /** @description Картинка: адрес `/api/images/…`. */
            image: string | null;
            /** @description События по отдельности (пусто, если событие одно и всё сказано в заголовке). */
            items: components["schemas"]["NotificationItem"][];
            kind: components["schemas"]["NotificationKind"];
            /**
             * @description Куда ведёт нажатие: путь сайта (`/album/{id}`, `/import`, `/rewind?year=2026`). Приложения
             *     открывают его как `dalywave://album/{id}`.
             */
            link: string | null;
            /** Format: date-time */
            readAt: string | null;
            title: string;
        };
        /** @description Одно событие внутри уведомления: у «3 новых релиза» — три альбома. */
        NotificationItem: {
            /** @description Обложка: адрес `/api/images/…`. */
            image: string | null;
            /** @description Путь сайта, например `/album/{id}`. */
            link: string | null;
            subtitle?: string;
            title: string;
        };
        /**
         * @description Вид уведомления.
         * @enum {string}
         */
        NotificationKind: "release" | "import" | "rewind" | "review" | "sources" | "test" | "friends" | "shared_playlist";
        /** @description Вид уведомлений включён или нет. */
        NotificationKindSetting: {
            enabled: boolean;
            kind: components["schemas"]["NotificationKind"];
        };
        /** @description Страница центра уведомлений: новые первыми. */
        NotificationPage: {
            items: components["schemas"]["Notification"][];
            /** @description Курсор следующей страницы (`?before=`); `null` — дальше ничего. */
            next: string | null;
            /**
             * Format: int64
             * @description Непрочитанных всего.
             */
            unread: number;
        };
        /** @description Отметить прочитанными: перечисленные или (пустой список) все. */
        NotificationReadRequest: {
            ids?: string[];
        };
        NotificationSettings: {
            /** @description Виды, доступные этому человеку (админские — только администраторам). */
            kinds: components["schemas"]["NotificationKindSetting"][];
            quiet: components["schemas"]["QuietHours"];
            /**
             * Format: int32
             * @description Часовой пояс для тихих часов: минуты к UTC (Москва — 180).
             */
            utcOffsetMin: number;
        };
        /** @description Изменить часть настроек. */
        NotificationSettingsPatch: {
            /** @description Только перечисленные виды; остальные как были. */
            kinds?: components["schemas"]["NotificationKindSetting"][] | null;
            quiet?: components["schemas"]["QuietHours"] | null;
            /** Format: int32 */
            utcOffsetMin?: number | null;
        };
        /** @description Событие WebSocket `notification.read` и ответ на отметку прочтения. */
        NotificationsRead: {
            all: boolean;
            /** @description Прочитанные; пусто вместе с `all` — все. */
            ids: string[];
            /**
             * Format: int64
             * @description Непрочитанных осталось.
             */
            unread: number;
        };
        OidcProvider: {
            /** @description Подпись кнопки входа, например `daly.rest`. */
            title: string;
        };
        /** @description `GET /api/overlay`: ссылки ещё нет — `link: null`. */
        OverlayInfo: {
            link: components["schemas"]["OverlayLink"] | null;
        };
        /** @description Ссылка на оверлей: `GET /api/overlay`, `POST /api/overlay/link`. */
        OverlayLink: {
            /** Format: date-time */
            createdAt: string;
            /** @description Плашка прячется, пока идёт приватная сессия. */
            hidePrivate: boolean;
            /** @description Адрес страницы для OBS без параметров вида: `https://…/overlay?k=dwo_…`. */
            url: string;
        };
        /** @description Что показать на плашке: `GET /api/overlay/now`, события `now` потока `/api/overlay/stream`. */
        OverlayNow: {
            /**
             * Format: date-time
             * @description Время сервера.
             */
            at: string;
            /** Format: int32 */
            durationMs: number;
            playing: boolean;
            /**
             * Format: int32
             * @description Позиция на момент `at`; пока играет, время идёт.
             */
            positionMs: number;
            track: components["schemas"]["OverlayTrack"] | null;
            /** @description Показывать плашку: играет и не приватная сессия. Иначе — тихо спрятать. */
            visible: boolean;
        };
        /** @description `PATCH /api/overlay`. */
        OverlayPatch: {
            hidePrivate?: boolean | null;
        };
        /** @description Трек на плашке. */
        OverlayTrack: {
            /** @description Артисты через запятую. */
            artist: string;
            /** @description Обложка 250×250 (путь `/api/images/…` или адрес Deezer). */
            cover: string | null;
            /** Format: uuid */
            id: string;
            title: string;
        };
        /**
         * @description Сессия для сопряжённого устройства (Apple Watch) от уже вошедшего приложения. Это не новый
         *     способ входа: человек уже вошёл через daly.rest на iPhone, часы получают дочернюю сессию.
         */
        PairRequest: {
            appVersion?: string | null;
            /** @description Имя для списка устройств; по умолчанию «Apple Watch». */
            deviceName?: string | null;
            /** @description Только `watchos`. */
            platform: components["schemas"]["Platform"];
        };
        PasswordLogin: {
            password: string;
        };
        /**
         * @description Трек выбран, но файла на сервере ещё нет.
         * @enum {string}
         */
        PendingStatus: "acquiring" | "waiting_for_downloader";
        PeopleList: {
            items: components["schemas"]["PersonCard"][];
        };
        /** @enum {string} */
        Permission: "play" | "library" | "admin" | "downloader";
        /** @description Человек в поиске и в приглашении: кем приходится. */
        PersonCard: {
            person: components["schemas"]["PersonRef"];
            relation: components["schemas"]["Relation"];
        };
        /** @description Человек на сервере: имя и аватар из daly.rest. */
        PersonRef: {
            avatarUrl: string | null;
            /** Format: uuid */
            id: string;
            name: string;
        };
        Ping: {
            id?: string | null;
        };
        /** @enum {string} */
        Platform: "web" | "macos" | "windows" | "linux" | "ios" | "ipados" | "android" | "watchos";
        /** @description Что играет: альбом, плейлист, волна… */
        PlayContext: {
            id?: string | null;
            title: string;
            type: components["schemas"]["ContextType"];
        };
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
        /** @description Снимок состояния активного устройства. Очередь — окно: ближайшие треки и немного истории. */
        PlaybackState: {
            /**
             * Format: date-time
             * @description Когда снят снимок — время сервера (его ставит сервер при приёме).
             */
            at?: string | null;
            context?: components["schemas"]["PlayContext"] | null;
            current?: components["schemas"]["QueueItem"] | null;
            /** Format: int32 */
            durationMs?: number;
            history?: components["schemas"]["QueueItem"][];
            manual?: components["schemas"]["QueueItem"][];
            muted?: boolean;
            /** @description Форма волны текущего трека, 0…1 — для полосы прокрутки на пульте. */
            peaks?: number[];
            pending?: components["schemas"]["PendingStatus"] | null;
            playing?: boolean;
            /**
             * Format: int32
             * @description Позиция на момент `at`.
             */
            positionMs?: number;
            repeat?: components["schemas"]["RepeatMode"];
            /** @description Перемешивание очереди альбома или плейлиста. */
            shuffle?: boolean;
            transition?: components["schemas"]["TransitionMode"];
            upcoming?: components["schemas"]["QueueItem"][];
            /**
             * Format: float
             * @description 0…1
             */
            volume?: number;
            wave?: components["schemas"]["WaveSettings"] | null;
            /** Format: uuid */
            waveSessionId?: string | null;
        };
        /**
         * @description Кем человек приходится плейлисту.
         * @enum {string}
         */
        PlaylistAccess: "owner" | "member" | "viewer";
        PlaylistDetail: {
            items: components["schemas"]["PlaylistItem"][];
            playlist: components["schemas"]["PlaylistSummary"];
        };
        /** @description Папка плейлистов в медиатеке и боковой панели. Вложенных папок нет. */
        PlaylistFolder: {
            /** @description Свёрнута ли — общее для всех устройств. */
            collapsed: boolean;
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            id: string;
            /**
             * Format: double
             * @description Место в «Своём порядке» среди папок и плейлистов вне папок.
             */
            position: number;
            title: string;
            /** Format: date-time */
            updatedAt: string;
        };
        PlaylistFolderPatch: {
            collapsed?: boolean | null;
            title?: string | null;
        };
        /** @description Приглашение в совместный плейлист по ссылке: что за плейлист, чей и вошёл ли уже. */
        PlaylistInvite: {
            /** @description Уже участник (или владелец): ссылка просто открывает плейлист. */
            joined: boolean;
            owner: components["schemas"]["PersonRef"];
            playlist: components["schemas"]["PlaylistSummary"];
        };
        PlaylistItem: {
            /** Format: date-time */
            addedAt: string;
            /**
             * Format: uuid
             * @description Кто добавил трек (видно в совместном плейлисте).
             */
            addedBy: string | null;
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
            /** @description Папки по «Своему порядку»; плейлисты ссылаются на них `folderId`. */
            folders?: components["schemas"]["PlaylistFolder"][];
            items: components["schemas"]["PlaylistSummary"][];
        };
        /**
         * @description Порядок плейлиста целиком: названные позиции — в этом порядке, остальные (добавленные тем
         *     временем) — за ними в прежнем порядке. Так «Отменить» возвращает порядок до сборки сета.
         */
        PlaylistOrder: {
            itemIds: string[];
        };
        PlaylistPatch: {
            description?: string | null;
            title?: string | null;
        };
        /**
         * @description Порядок плейлистов в медиатеке и боковой панели. Закреплённые — всегда первыми.
         * @enum {string}
         */
        PlaylistSort: "recent" | "title" | "custom";
        PlaylistSummary: {
            /** @description Кем плейлист приходится смотрящему: свой, участник совместного, плейлист друга. */
            access?: components["schemas"]["PlaylistAccess"];
            /** @description Совместный: участники добавляют и переставляют треки. */
            collaborative?: boolean;
            /** @description Обложки первых альбомов плейлиста (до четырёх) — для мозаики. */
            covers: components["schemas"]["ImageSet"][];
            /** Format: date-time */
            createdAt: string;
            description: string;
            /** Format: int64 */
            durationMs: number;
            /**
             * Format: uuid
             * @description Папка, в которой лежит плейлист; `null` — вне папок.
             */
            folderId?: string | null;
            /** @description Друзья видят плейлист на странице владельца. */
            friendsCanSee?: boolean;
            /** @description Кто собрал плейлист вида `mix`: `daily_mix`, `discover_weekly`, `on_repeat`, `repeat_rewind`, `rewind`… */
            generator?: string | null;
            /** Format: uuid */
            id: string;
            image?: components["schemas"]["ImageSet"] | null;
            kind: components["schemas"]["PlaylistKind"];
            /** @description Участники совместного (без владельца). */
            members?: components["schemas"]["PersonRef"][];
            owner: components["schemas"]["PersonRef"] | null;
            pinned: boolean;
            /**
             * Format: date-time
             * @description Когда плейлист в последний раз играл — для порядка «Недавние».
             */
            playedAt?: string | null;
            /**
             * Format: double
             * @description Место в «Своём порядке» (дробное, по возрастанию): вне папок — общая шкала с папками.
             */
            position?: number;
            title: string;
            /** Format: int64 */
            tracksCount: number;
            /** Format: date-time */
            updatedAt: string;
        };
        /** @description Открыть или скрыть плейлист от друзей. */
        PlaylistVisibility: {
            friendsCanSee: boolean;
        };
        Pong: {
            id: string | null;
        };
        /** @description Приватность прослушиваний. */
        Privacy: {
            /**
             * @description Приватная сессия идёт: друзья не видят, что играет, а сыгранное в ней не попадает в
             *     «последнее» и в топ на странице друга.
             */
            privateSession: boolean;
            /**
             * Format: date-time
             * @description Когда сессия закончится сама; `null` — до выключения (или сессии нет).
             */
            privateUntil: string | null;
            /** @description «Показывать друзьям, что я слушаю». */
            shareListening: boolean;
        };
        PrivacyPatch: {
            privateSession?: components["schemas"]["PrivateSessionMode"] | null;
            shareListening?: boolean | null;
        };
        /**
         * @description Приватная сессия: на 6 часов или до выключения.
         * @enum {string}
         */
        PrivateSessionMode: "off" | "hours6" | "until_off";
        /**
         * @description Канал доставки push.
         * @enum {string}
         */
        PushChannel: "apns" | "webpush" | "fcm";
        /** @description Устройство, подписанное на push. */
        PushDevice: {
            channel: components["schemas"]["PushChannel"];
            /** Format: date-time */
            createdAt: string;
            /** @description Подписка этой сессии (этого браузера или приложения). */
            current: boolean;
            /** Format: uuid */
            id: string;
            /** Format: date-time */
            lastSentAt: string | null;
            name: string;
        };
        /** @description Какие каналы push есть на сервере и какие устройства человека подписаны. */
        PushInfo: {
            /** @description APNs настроен: приложению на iPhone и iPad есть смысл просить разрешение. */
            apns: boolean;
            devices: components["schemas"]["PushDevice"][];
            fcm: boolean;
            /** @description Открытый ключ VAPID (base64url) для `pushManager.subscribe`; `null` — Web Push на сервере выключен. */
            webPushKey: string | null;
        };
        PushRegistered: {
            /** Format: uuid */
            id: string;
        };
        /**
         * @description Подписать это устройство на push. Повтор с тем же токеном обновляет подписку (токен APNs стоит
         *     присылать при каждом запуске приложения).
         */
        PushRegistration: {
            channel: components["schemas"]["PushChannel"];
            environment?: components["schemas"]["ApnsEnvironment"] | null;
            /** @description Как назвать устройство в списке («iPhone», «Chrome»). По умолчанию — по сессии. */
            name?: string | null;
            subscription?: components["schemas"]["WebPushSubscription"] | null;
            /** @description APNs и FCM: токен устройства (у APNs — hex). */
            token?: string | null;
            /**
             * Format: int32
             * @description Часовой пояс устройства (минуты к UTC) — для тихих часов.
             */
            utcOffsetMin?: number | null;
        };
        /** @description Ответ на пробное уведомление. */
        PushTest: {
            /**
             * Format: int64
             * @description На сколько устройств ушёл push.
             */
            devices: number;
        };
        /** @description Отписать устройство: токен APNs/FCM или адрес подписки браузера. */
        PushUnregister: {
            endpoint?: string | null;
            token?: string | null;
        };
        /** @description Элемент очереди: `uid` — id элемента у активного устройства (команды ссылаются на него). */
        QueueItem: {
            /** @description «Почему этот трек» у волны. */
            reason?: string | null;
            source: components["schemas"]["QueueSource"];
            track: components["schemas"]["Track"];
            uid: string;
        };
        /**
         * @description Откуда элемент очереди.
         * @enum {string}
         */
        QueueSource: "manual" | "context" | "wave";
        /**
         * @description Тихие часы: в это время push не приходят — они ждут утра (центр уведомлений и окно приложения
         *     получают их сразу). Время — минуты от полуночи по часовому поясу человека; `from > to` — через полночь.
         */
        QuietHours: {
            enabled: boolean;
            /**
             * Format: int32
             * @description 0…1439, например 1380 — 23:00.
             */
            from: number;
            /**
             * Format: int32
             * @description 0…1439, например 480 — 08:00.
             */
            to: number;
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
        /** @description Слушатель в выборе раздела: лайки, прослушивания и граней вкуса. */
        RecoListener: {
            /** Format: int64 */
            facets: number;
            /** Format: uuid */
            id: string;
            /** Format: int64 */
            likes: number;
            name: string;
            /** Format: int64 */
            plays: number;
        };
        /** @description Раздел «Рекомендации»: вкус выбранного слушателя и общий индекс каталога. */
        RecoOverview: {
            /** @description Грани вкуса слушателя, самые весомые первыми. */
            facets: components["schemas"]["FacetView"][];
            /** @description Последние показы слушателю, новые первыми. */
            impressions: components["schemas"]["ImpressionView"][];
            /** @description Общий индекс каталога: один на всех слушателей. */
            index: components["schemas"]["IndexStats"];
            /** @description Кого можно выбрать: тот, кто смотрит, и все, у кого есть лайки или прослушивания. */
            listeners: components["schemas"]["RecoListener"][];
            /** @description «Миксы дня» и «Открытия недели» слушателя. */
            mixes: components["schemas"]["MixView"][];
            ml: components["schemas"]["MlStatus"];
            /** @description Версия, которая собирает поток: `rules-1`, пока своей модели нет. */
            modelVersion: string | null;
            /** @description Версии сети вкуса с метриками на отложенных данных, новые первыми (и правила). */
            models: components["schemas"]["ModelVersionView"][];
            /** @description Заготовки волны слушателя по сочетаниям настроек. */
            pools: components["schemas"]["WavePoolView"][];
            /** @description Последние сессии волны и радио слушателя. */
            sessions: components["schemas"]["WaveSessionView"][];
            settings: components["schemas"]["RecoSettings"];
            /** @description Последнее обучение сети вкуса. */
            training: components["schemas"]["ModelTraining"];
            /** @description Чей вкус показан. */
            user: components["schemas"]["RecoListener"];
        };
        RecoRun: {
            task: components["schemas"]["RecoTask"];
            /**
             * Format: uuid
             * @description Для `taste`: только этот слушатель; без него — все.
             */
            userId?: string | null;
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
        RecoTask: "scout" | "taste" | "analysis" | "model";
        /** @description Распознанная песня из истории. */
        Recognition: {
            artist: string;
            artworkUrl: string | null;
            /** Format: uuid */
            id: string;
            isrc: string | null;
            /** Format: date-time */
            recognizedAt: string;
            /** @description Как прислал Shazam. */
            title: string;
            track: components["schemas"]["Track"] | null;
        };
        RecognitionPage: {
            items: components["schemas"]["Recognition"][];
            nextCursor: string | null;
        };
        /** @description Что узнал Shazam. */
        RecognizeRequest: {
            appleMusicId?: string | null;
            /** @description Артист (до 300 символов). */
            artist: string;
            /** @description Обложка от Shazam, только `https://`. */
            artworkUrl?: string | null;
            /** @description ISRC записи: по нему трек находится точнее всего. */
            isrc?: string | null;
            shazamId?: string | null;
            /** @description Название (до 300 символов). */
            title: string;
        };
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
         * @description Кем человек приходится тому, кто спрашивает.
         * @enum {string}
         */
        Relation: "me" | "none" | "friend" | "outgoing" | "incoming";
        /**
         * @description Загрузчик бросает задание (приложение закрывают посреди скачивания): задание сразу возвращается
         *     в очередь, попытка не тратится.
         */
        ReleaseRequest: {
            /** @description Почему: `quit`, `pause`, `control` — для журнала задачи. */
            reason?: string | null;
            workerId?: string | null;
        };
        ReleaseUpload: {
            /** Format: date-time */
            expiresAt: string;
            /** @description Ключ файла в хранилище */
            key: string;
            /** @description Подписанная ссылка: `PUT` с телом файла */
            url: string;
        };
        ReleaseUploadRequest: {
            /** @description `aarch64`, `x86_64`, `i686`, `armv7` */
            arch: string;
            /** @description Имя файла обновления, например `DalyWave.app.tar.gz` или `DalyWave_0.2.0_x64-setup.exe` */
            fileName: string;
            /** @description `darwin`, `windows`, `linux` */
            target: string;
            /** @description SemVer, например `0.2.0` */
            version: string;
        };
        /** @description Дискография по типам релизов, новые первыми. */
        Releases: {
            albums: components["schemas"]["AlbumSummary"][];
            compilations: components["schemas"]["AlbumSummary"][];
            eps: components["schemas"]["AlbumSummary"][];
            singles: components["schemas"]["AlbumSummary"][];
        };
        /** @description Команда пульта. Поле `action`, параметры — рядом с ним. */
        RemoteCommand: {
            /** @enum {string} */
            action: "play";
        } | {
            /** @enum {string} */
            action: "pause";
        } | {
            /** @enum {string} */
            action: "toggle";
        } | {
            /** @enum {string} */
            action: "next";
        } | {
            /** @enum {string} */
            action: "previous";
        } | (components["schemas"]["RemoteSeek"] & {
            /** @enum {string} */
            action: "seek";
        }) | (components["schemas"]["RemoteVolume"] & {
            /** @enum {string} */
            action: "setVolume";
        }) | (components["schemas"]["RemoteMuted"] & {
            /** @enum {string} */
            action: "setMuted";
        }) | (components["schemas"]["RemoteTransition"] & {
            /** @enum {string} */
            action: "setTransition";
        }) | (components["schemas"]["RemoteShuffle"] & {
            /** @enum {string} */
            action: "setShuffle";
        }) | (components["schemas"]["RemoteRepeat"] & {
            /** @enum {string} */
            action: "setRepeat";
        }) | {
            /** @enum {string} */
            action: "dislike";
        } | {
            /** @enum {string} */
            action: "notNow";
        } | {
            /** @enum {string} */
            action: "moreLikeThis";
        } | (components["schemas"]["RemotePlayTracks"] & {
            /** @enum {string} */
            action: "playTracks";
        }) | (components["schemas"]["RemoteWave"] & {
            /** @enum {string} */
            action: "startWave";
        }) | (components["schemas"]["RemoteWave"] & {
            /** @enum {string} */
            action: "setWave";
        }) | (components["schemas"]["RemoteQueueItem"] & {
            /** @enum {string} */
            action: "jumpTo";
        }) | (components["schemas"]["RemoteQueueItem"] & {
            /** @enum {string} */
            action: "remove";
        }) | (components["schemas"]["RemoteTrack"] & {
            /** @enum {string} */
            action: "playNext";
        }) | (components["schemas"]["RemoteTrack"] & {
            /** @enum {string} */
            action: "enqueue";
        }) | {
            /** @enum {string} */
            action: "clearManual";
        } | (components["schemas"]["RemoteMove"] & {
            /** @enum {string} */
            action: "moveManual";
        });
        RemoteMove: {
            /** Format: int32 */
            from: number;
            /** Format: int32 */
            to: number;
        };
        RemoteMuted: {
            muted: boolean;
        };
        /** @description Играть список на устройстве: треки — по id (устройство загрузит их само). */
        RemotePlayTracks: {
            context: components["schemas"]["PlayContext"];
            /** Format: int32 */
            index?: number;
            shuffle?: boolean;
            trackIds: string[];
        };
        RemoteQueueItem: {
            uid: string;
        };
        RemoteRepeat: {
            mode: components["schemas"]["RepeatMode"];
        };
        RemoteSeek: {
            /** Format: int32 */
            positionMs: number;
        };
        RemoteShuffle: {
            shuffle: boolean;
        };
        RemoteTrack: {
            track: components["schemas"]["Track"];
        };
        RemoteTransition: {
            mode: components["schemas"]["TransitionMode"];
        };
        RemoteVolume: {
            /**
             * Format: float
             * @description 0…1
             */
            volume: number;
        };
        RemoteWave: {
            settings?: components["schemas"]["WaveSettings"] | null;
        };
        /** @description Убрать несколько позиций разом (выбранные в списке). */
        RemovePlaylistItems: {
            itemIds: string[];
        };
        /**
         * @description Повтор: выключен, вся очередь по кругу (альбом, плейлист) или один трек.
         * @enum {string}
         */
        RepeatMode: "off" | "all" | "one";
        /**
         * @description Скачать запись заново: по ссылке (YouTube, SoundCloud, Bandcamp…) или поиском, если ссылки нет.
         *     Файл, выбранный человеком, заменяет основной.
         */
        ReplaceAudio: {
            url?: string | null;
        };
        /**
         * @description Что сделать со спорной (или неудачной) записью.
         * @enum {string}
         */
        ResolveAction: "choose" | "url" | "unavailable" | "retry";
        /** @description Ссылка или текст, в котором она есть (так приходит «Поделиться» из других приложений). */
        ResolveLinkRequest: {
            url: string;
        };
        ResolveRequest: {
            action: components["schemas"]["ResolveAction"];
            /** @description Для `url`. */
            url?: string | null;
            /** @description Для `choose`: id видео YouTube (11 символов). */
            videoId?: string | null;
        };
        /** @description Найденное по ссылке: заполнено поле своего вида (`track`, `album`, `artist` или `playlistId`). */
        ResolvedLink: {
            album: components["schemas"]["AlbumSummary"] | null;
            artist: components["schemas"]["ArtistSummary"] | null;
            kind: components["schemas"]["ShareKind"];
            /** Format: uuid */
            playlistId: string | null;
            /** @description Текст, по которому искали в каталоге (для ссылок не на Deezer и не на DalyWave). */
            query: string | null;
            source: components["schemas"]["LinkSource"];
            track: components["schemas"]["Track"] | null;
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
        /** @description `GET /api/rewind/{year}`. */
        Rewind: {
            /** Format: int64 */
            artists: number;
            /** @description Год закончился: итоги окончательные. Иначе — «по сей день». */
            complete: boolean;
            /** Format: date-time */
            computedAt: string;
            /**
             * Format: int64
             * @description Дней, когда что-то играло.
             */
            days: number;
            /**
             * Format: int64
             * @description Дней в году на сегодня (для «N из M дней»).
             */
            daysInRange: number;
            discoveries: components["schemas"]["RewindDiscovery"][];
            firstTrack: components["schemas"]["RewindFirst"] | null;
            genres: components["schemas"]["RewindGenre"][];
            /**
             * Format: int64
             * @description Лайков за год.
             */
            likes: number;
            /** Format: int64 */
            minutes: number;
            /** @description Двенадцать месяцев по порядку. */
            months: components["schemas"]["RewindMonth"][];
            moods: components["schemas"]["RewindMood"][];
            /**
             * Format: int64
             * @description Артистов, которых до этого года не слушали.
             */
            newArtists: number;
            personality: components["schemas"]["RewindPersonality"];
            /**
             * Format: uuid
             * @description Плейлист «Твои лучшие треки {год}», если уже сохранён.
             */
            playlistId: string | null;
            /**
             * Format: int64
             * @description Прослушиваний (от 30 секунд или до конца).
             */
            plays: number;
            /** @description Сезон итогов: с 1 декабря и после конца года — карточка на главной. */
            season: boolean;
            streak: components["schemas"]["RewindStreak"] | null;
            topAlbums: components["schemas"]["RewindAlbum"][];
            topArtists: components["schemas"]["RewindArtist"][];
            topDay: components["schemas"]["RewindDay"] | null;
            topTracks: components["schemas"]["RewindTrack"][];
            trackOfYear: components["schemas"]["RewindTrackOfYear"] | null;
            /** Format: int64 */
            tracks: number;
            /**
             * Format: int64
             * @description Сессий «Моей волны» и радио за год.
             */
            waveSessions: number;
            /** Format: int32 */
            year: number;
        };
        RewindAlbum: {
            album: components["schemas"]["AlbumSummary"];
            /** Format: int64 */
            minutes: number;
            /** Format: int64 */
            plays: number;
        };
        RewindArtist: {
            artist: components["schemas"]["ArtistSummary"];
            /** Format: int64 */
            minutes: number;
            /** Format: int64 */
            plays: number;
        };
        /** @description Звук слайда историй: что играет и с какого места. */
        RewindCue: {
            /**
             * @description Слайд: `intro`, `minutes`, `days`, `first`, `tracks`, `toy`, `artists`, `albums`, `sound`,
             *     `discoveries`, `personality`, `summary`. Слайда нет в списке — играет прежний трек.
             */
            slide: string;
            /** @description Откуда место: `analysis` — по кривой энергии полного анализа, `estimate` — 30 % трека. */
            startFrom: string;
            /**
             * Format: int64
             * @description С какого места играть: припев или самая энергичная часть.
             */
            startMs: number;
            track: components["schemas"]["Track"];
        };
        /** @description День, когда слушали больше всего. */
        RewindDay: {
            /**
             * Format: date
             * @description Дата по времени слушателя.
             */
            date: string;
            /** Format: int64 */
            minutes: number;
            /** Format: int64 */
            plays: number;
            track: components["schemas"]["Track"] | null;
            /**
             * Format: int64
             * @description Сколько раз он играл в тот день.
             */
            trackPlays: number;
        };
        /** @description Открытие года: артист, которого впервые услышали в этом году, и он стал любимым. */
        RewindDiscovery: {
            artist: components["schemas"]["ArtistSummary"];
            /**
             * Format: date
             * @description Первое прослушивание (дата по времени слушателя).
             */
            firstHeard: string;
            /**
             * Format: int64
             * @description Лайкнутых треков артиста.
             */
            likes: number;
            /** Format: int64 */
            plays: number;
        };
        /** @description `GET /api/admin/rewind`: «Итоги года» как событие сервера. */
        RewindEvent: {
            /**
             * Format: date-time
             * @description Когда закроется само (режим «Автоматически», пока открыто).
             */
            closesAt: string | null;
            mode: components["schemas"]["RewindMode"];
            /**
             * Format: int32
             * @description Какой год открыт прямо сейчас; `null` — закрыто.
             */
            openYear: number | null;
            /**
             * Format: date-time
             * @description Ближайшее автоматическое открытие (режим «Автоматически», пока закрыто).
             */
            opensAt: string | null;
            style: components["schemas"]["RewindStyleSetting"];
            /** Format: date-time */
            updatedAt: string;
            /**
             * Format: int32
             * @description Год для режима «Открыто сейчас».
             */
            year: number | null;
            /** @description Годы с прослушиваниями на сервере — для выбора года (новые первыми). */
            years: number[];
        };
        /** @description `PATCH /api/admin/rewind`. */
        RewindEventPatch: {
            mode?: components["schemas"]["RewindMode"] | null;
            style?: components["schemas"]["RewindStyleSetting"] | null;
            /**
             * Format: int32
             * @description Год для «Открыто сейчас»; без него — текущий.
             */
            year?: number | null;
        };
        RewindFirst: {
            /** Format: date-time */
            at: string;
            track: components["schemas"]["Track"];
        };
        /** @description Жанр года: стиль Discogs из анализа звука (по-русски), а без анализа — жанр Deezer. */
        RewindGenre: {
            /** @description Как есть: «Hip Hop---Trap» или «Rap/Hip Hop». */
            id: string;
            /** @description «Трэп». */
            name: string;
            /**
             * Format: float
             * @description Доля времени прослушивания, 0..1.
             */
            share: number;
        };
        /**
         * @description Когда итоги открыты: `auto` — с 1 декабря по 15 января (в декабре — за этот год, в январе — за
         *     прошлый), `open` — открыты сейчас за выбранный год, `closed` — закрыты.
         * @enum {string}
         */
        RewindMode: "auto" | "open" | "closed";
        RewindMonth: {
            /** Format: int64 */
            minutes: number;
            /**
             * Format: int32
             * @description 1..12
             */
            month: number;
        };
        /** @description Настроение года по анализу звука: `happy`, `sad`, `aggressive`, `relaxed`, `party`. */
        RewindMood: {
            id: string;
            /**
             * Format: float
             * @description Средняя вероятность по времени прослушивания, 0..1.
             */
            score: number;
        };
        /** @description `GET /api/rewind/{year}/more`: «Ещё послушать» — новое для слушателя, похожее на любимое за год. */
        RewindMore: {
            items: components["schemas"]["Track"][];
            /**
             * Format: uuid
             * @description Плейлист «Ещё послушать {год}», если уже сохранён.
             */
            playlistId: string | null;
        };
        /**
         * @description Характер слушателя: `night_owl` (ночной слушатель), `early_bird` (утро), `explorer` (много нового),
         *     `loyal` (переслушивает любимое), `wave_rider` (слушает волну), `balanced` (всего понемногу).
         */
        RewindPersonality: {
            /** @description Доля времени по часам суток (24 значения, 0 — с полуночи до часа), по времени слушателя. */
            hours: number[];
            id: string;
            /**
             * Format: float
             * @description Доля времени утром (5–12).
             */
            morningShare: number;
            /**
             * Format: float
             * @description Доля времени ночью (23–5).
             */
            nightShare: number;
            /**
             * Format: float
             * @description Доля треков года, которые до этого года не играли.
             */
            noveltyShare: number;
            /**
             * Format: int32
             * @description Час, когда слушали больше всего, 0..23.
             */
            peakHour: number;
            /** @description Время суток с наибольшей долей: `morning`, `day`, `evening`, `night`. */
            peakPart: string;
            /**
             * Format: float
             * @description Доля прослушиваний-повторов: 1 − треков / прослушиваний.
             */
            repeatShare: number;
            /**
             * Format: float
             * @description Доля времени в «Моей волне» и радио.
             */
            waveShare: number;
        };
        /**
         * @description `GET /api/rewind/{year}/soundtrack`: музыка историй. Треки, которых ещё нет на сервере, ставятся
         *     на скачивание вне очереди — к своему слайду они успевают.
         */
        RewindSoundtrack: {
            /** @description По порядку слайдов. */
            cues: components["schemas"]["RewindCue"][];
            /** Format: int32 */
            year: number;
        };
        RewindStreak: {
            /**
             * Format: int64
             * @description Дней подряд.
             */
            days: number;
            /** Format: date */
            from: string;
            /** Format: date */
            to: string;
        };
        /**
         * @description Стиль историй: `wave` — «Волна года», `spread` — «Разворот», `record` — «Пластинка».
         * @enum {string}
         */
        RewindStyle: "wave" | "spread" | "record";
        /**
         * @description Стиль в настройках сервера: один для всех или `random` — случайный для каждого слушателя.
         * @enum {string}
         */
        RewindStyleSetting: "wave" | "spread" | "record" | "random";
        RewindTrack: {
            /** Format: int64 */
            minutes: number;
            /**
             * Format: int64
             * @description Прослушиваний (от 30 секунд или до конца).
             */
            plays: number;
            track: components["schemas"]["Track"];
        };
        /** @description Трек года: самый слушаемый, с историей — когда впервые и в каком месяце его было больше всего. */
        RewindTrackOfYear: {
            /**
             * Format: date
             * @description Первое прослушивание в году.
             */
            firstPlayed: string;
            /** Format: int64 */
            minutes: number;
            /**
             * Format: int32
             * @description 1..12
             */
            peakMonth: number;
            /** Format: int64 */
            plays: number;
            track: components["schemas"]["Track"];
        };
        RewindYear: {
            /** Format: int64 */
            minutes: number;
            /** Format: int32 */
            year: number;
        };
        /** @description `GET /api/rewind`: годы, за которые есть прослушивания (новые первыми), и текущий год слушателя. */
        RewindYears: {
            /**
             * Format: int32
             * @description Год итогов: открытый событием, а у администратора при закрытом — текущий.
             */
            current: number;
            /** @description Итоги открыты: карточки и истории видны. Закрытые видит только администратор (предпросмотр). */
            open: boolean;
            /**
             * Format: date-time
             * @description Когда итоги откроются сами (режим «Автоматически», пока закрыто) — для напоминания в приложении.
             */
            opensAt: string | null;
            /** @description Сезон итогов текущего года (с 1 декабря). */
            season: boolean;
            /** @description Итоги года `current` уже посмотрены: карточка «Твои итоги готовы» больше не нужна. */
            seen: boolean;
            /** @description Стиль историй этого слушателя. */
            style: components["schemas"]["RewindStyle"];
            years: components["schemas"]["RewindYear"][];
        };
        /**
         * @description Роль в DalyWave. Роли совмещаются; человек без ролей видит экран «ждём доступа».
         * @enum {string}
         */
        Role: "admin" | "listener";
        /** @description Сохранить предпросмотр обычным плейлистом. */
        SaveDescribed: {
            brief: components["schemas"]["DescribeBrief"];
            description?: string | null;
            title: string;
            trackIds: string[];
            /** @description Раз в неделю менять прослушанное на новое в том же духе. */
            weekly?: boolean;
        };
        /** @description `POST /api/rewind/{year}/more`: сохранить показанное плейлистом. */
        SaveRewindMore: {
            /** @description Треки по порядку, до 100. */
            trackIds: string[];
        };
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
        SeedKind: "track" | "artist" | "album" | "playlist" | "text" | "year";
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
        }) | (components["schemas"]["ConnectDevices"] & {
            /** @enum {string} */
            type: "connect.devices";
        }) | (components["schemas"]["ConnectState"] & {
            /** @enum {string} */
            type: "connect.state";
        }) | (components["schemas"]["ConnectActive"] & {
            /** @enum {string} */
            type: "connect.active";
        }) | (components["schemas"]["ConnectCommand"] & {
            /** @enum {string} */
            type: "connect.command";
        }) | (components["schemas"]["ConnectAck"] & {
            /** @enum {string} */
            type: "connect.ack";
        }) | (components["schemas"]["ConnectHandoff"] & {
            /** @enum {string} */
            type: "connect.handoff";
        }) | (components["schemas"]["JamChanged"] & {
            /** @enum {string} */
            type: "jam.changed";
        }) | (components["schemas"]["JamNowEvent"] & {
            /** @enum {string} */
            type: "jam.now";
        }) | (components["schemas"]["JamReaction"] & {
            /** @enum {string} */
            type: "jam.reaction";
        }) | (components["schemas"]["Notification"] & {
            /** @enum {string} */
            type: "notification.new";
        }) | (components["schemas"]["NotificationsRead"] & {
            /** @enum {string} */
            type: "notification.read";
        }) | (components["schemas"]["FriendsChange"] & {
            /** @enum {string} */
            type: "friends.changed";
        }) | (components["schemas"]["FriendActivityEvent"] & {
            /** @enum {string} */
            type: "friends.activity";
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
        ServerSettings: {
            /** @description Что докачивать по новой подписке, если человек не выбрал сам. */
            followPolicy: components["schemas"]["DownloadPolicy"];
            /** Format: date-time */
            updatedAt: string;
            /**
             * Format: int32
             * @description Часовой пояс владельца в минутах от UTC: окна работы полос, время суток у граней вкуса.
             */
            utcOffsetMin: number;
        };
        /** @description Изменение настроек: только переданные поля. */
        ServerSettingsPatch: {
            followPolicy?: components["schemas"]["DownloadPolicy"] | null;
            /**
             * Format: int32
             * @description −720…840.
             */
            utcOffsetMin?: number | null;
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
        /** @description Сет применён: новый порядок и прежний — для «Отменить». */
        SetApplied: {
            /** @description Порядок позиций до сборки: вернуть — `PUT /api/playlists/{id}/order`. */
            previousItemIds: string[];
            set: components["schemas"]["SetPreview"];
        };
        /**
         * @description «Собрать как сет»: как идёт энергия.
         * @enum {string}
         */
        SetCurve: "rise" | "wave" | "flat";
        SetEntry: {
            /**
             * Format: float
             * @description Энергия трека 0…1 — для линии энергии сета.
             */
            energy?: number | null;
            fit?: components["schemas"]["SetFit"] | null;
            /** Format: uuid */
            itemId: string;
            /** @description Поставлен по анализу; `false` — анализа нет, трек в конце в прежнем порядке. */
            placed: boolean;
            /** Format: uuid */
            trackId: string;
        };
        /**
         * @description Как трек переходит из предыдущего — мягкая подсказка в списке.
         * @enum {string}
         */
        SetFit: "in_key" | "smooth" | "lift" | "calm" | "switch";
        /** @description Настройки сборки сета. */
        SetOptions: {
            curve?: components["schemas"]["SetCurve"];
            /**
             * Format: uuid
             * @description Позиция, с которой начать сет; без неё первый выбирается сам.
             */
            firstItemId?: string | null;
            /** @description Не ставить подряд треки одного исполнителя. */
            spreadArtists?: boolean;
        };
        /** @description Порядок сета: новый порядок позиций и что изменится. */
        SetPreview: {
            /** @description Новый порядок отличается от нынешнего. */
            changed: boolean;
            curve: components["schemas"]["SetCurve"];
            /** @description Все позиции плейлиста в новом порядке. */
            entries: components["schemas"]["SetEntry"][];
            /** @description Плейлист собирается сам (микс): применить можно к своей копии. */
            readOnly: boolean;
            /** Format: int64 */
            smoothAfter: number;
            /**
             * Format: int64
             * @description Переходов, которые сводятся плавно: сейчас и в новом порядке.
             */
            smoothBefore: number;
            /**
             * Format: int64
             * @description Треков без анализа (они в конце).
             */
            unplaced: number;
        };
        /** @description Карточка для ссылки: то же видят превью в мессенджерах и статус в Discord. */
        ShareCard: {
            /** Format: uuid */
            id: string;
            /** @description Публичный адрес обложки (у Deezer), который открывается без входа; `null` — обложки нет. */
            image: string | null;
            kind: components["schemas"]["ShareKind"];
            /**
             * @description Трек — исполнители через запятую, альбом — главный исполнитель, плейлист — описание,
             *     артист — пусто.
             */
            subtitle: string;
            title: string;
            /** @description Ссылка на страницу в DalyWave. */
            url: string;
        };
        /**
         * @description Что открывает ссылка `https://wave.daly.rest/{kind}/{id}`.
         * @enum {string}
         */
        ShareKind: "track" | "album" | "artist" | "playlist";
        Shelf: {
            /**
             * @description Постоянный id полки: `continue`, `daily-mixes`, `new-releases`, `discover-weekly`, `recent-likes`,
             *     `forgotten-favorites`, `top-artists`, `release-radar`.
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
        ShelfKind: "continue" | "new_releases" | "recent_likes" | "top_artists" | "daily_mixes" | "discover_weekly" | "forgotten_favorites" | "release_radar";
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
        /** @description «Проверить скачивание»: поставить трек вне очереди и посмотреть, как его скачают. */
        SourceCheck: {
            /**
             * Format: uuid
             * @description Какой трек; без него — свежий лайк без файла, иначе любой трек каталога без файла.
             */
            trackId?: string | null;
        };
        /** @description Попытки скачать за день (UTC) у всех исполнителей. */
        SourceDay: {
            /** @description `2026-11-11`. */
            day: string;
            /**
             * Format: int64
             * @description Скачано, но не совпало — ждёт решения в «Спорных».
             */
            disputed: number;
            /**
             * Format: int64
             * @description Сбои: повтор, окончательные, отложенные из-за исполнителя.
             */
            failed: number;
            /**
             * Format: int64
             * @description Файл принят.
             */
            ok: number;
            /**
             * Format: int64
             * @description Записи нет в источниках.
             */
            unavailable: number;
        };
        /** @description Раздел «Источники»: кто качает аудио. */
        Sources: {
            /** @description Последние 14 дней, старые первыми; дни без попыток — с нулями. */
            daily: components["schemas"]["SourceDay"][];
            downloaders: components["schemas"]["Downloader"][];
            server: components["schemas"]["ServerFetcher"];
        };
        StatusCount: {
            /** Format: int64 */
            count: number;
            status: components["schemas"]["AudioStatus"];
        };
        StorageArtist: {
            artist: components["schemas"]["ArtistRef"];
            /** Format: int64 */
            bytes: number;
            /** Format: int64 */
            files: number;
        };
        /** @description Прирост звука за день (UTC). */
        StorageDay: {
            /** Format: int64 */
            bytes: number;
            /** @description `2026-11-11`. */
            day: string;
            /** Format: int64 */
            files: number;
        };
        /** @description Сколько места занимает группа файлов. */
        StorageGroup: {
            /** Format: int64 */
            bytes: number;
            /** Format: int64 */
            files: number;
            /** @description Для видов: `audio`, `covers`, `photos`; для звука — кодек или источник. */
            key: string;
        };
        /** @description Раздел «Хранилище». */
        StorageOverview: {
            /** @description Самые «тяжёлые» артисты по основным файлам их записей, до 12. */
            artists: components["schemas"]["StorageArtist"][];
            /** @description Звук по кодекам. */
            codecs: components["schemas"]["StorageGroup"][];
            /** @description Прирост звука за последние 30 дней, по дням. */
            growth: components["schemas"]["StorageDay"][];
            /** @description Звук, обложки, фото артистов. */
            kinds: components["schemas"]["StorageGroup"][];
            /** Format: int64 */
            replacedBytes: number;
            /**
             * Format: int64
             * @description Файлы, вытесненные лучшими копиями: занимают место, но не играются.
             */
            replacedFiles: number;
            /** @description Звук по источникам. */
            sources: components["schemas"]["StorageGroup"][];
            /** Format: int64 */
            totalBytes: number;
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
        /**
         * @description Какой файл записи отдать.
         * @enum {string}
         */
        StreamQuality: "high" | "normal" | "saver";
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
            lossless?: boolean;
            /**
             * Format: float
             * @description Интегральная громкость (EBU R128) — для выравнивания.
             */
            loudnessLufs: number | null;
            /** @description Для `<audio>` и `MediaSource`: `audio/mp4; codecs="opus"`. */
            mimeType: string;
            /** @description Какое качество отдано: при `saver` до готовности копии — `normal`. */
            quality?: components["schemas"]["StreamQuality"];
            /** Format: float */
            truePeakDb: number | null;
            /** @description Подписанная ссылка на файл в хранилище. */
            url: string;
            /** @description Огибающая для полосы прокрутки: 512 точек, 0..1000. */
            waveform: number[] | null;
        };
        /** @description «Умное перемешивание»: похожее на весь список (альбом, плейлист), чтобы подмешать в очередь. */
        SuggestRequest: {
            /** @description Что ещё не предлагать: уже подмешанное и убранное из очереди, до 500. */
            exclude?: string[];
            /**
             * Format: int32
             * @description 1–50, по умолчанию 20.
             */
            limit?: number | null;
            /** @description Треки списка, до 500: по их звучанию ищется похожее, сами они не предлагаются. */
            trackIds: string[];
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
        /** @description Трек набора волны этого устройства: любимое (`favorite`) или новое (`fresh`). */
        SyncDeviceSetItem: {
            /** Format: uuid */
            deviceId: string;
            kind: string;
            /** Format: int64 */
            rev: number;
            /** Format: float */
            score: number;
            /** Format: uuid */
            trackId: string;
        };
        SyncDiscographyPayload: {
            /** Format: uuid */
            artistId: string;
        };
        /**
         * @description Таблица локальной базы.
         * @enum {string}
         */
        SyncEntity: "artists" | "albums" | "tracks" | "recordings" | "likedTracks" | "savedAlbums" | "followedArtists" | "blockedItems" | "playlists" | "playlistItems" | "deviceDownloads" | "deviceSets" | "trackFeatures" | "wavePresets" | "playlistFolders" | "settings";
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
            /** @description Только этого устройства (вход токеном приложения). */
            deviceSets?: components["schemas"]["SyncDeviceSetItem"][];
            followedArtists: components["schemas"]["SyncFollowedArtist"][];
            /** @description Есть ещё изменения: запросить следующую страницу сразу. */
            hasMore: boolean;
            likedTracks: components["schemas"]["SyncLikedTrack"][];
            playlistFolders?: components["schemas"]["SyncPlaylistFolder"][];
            playlistItems: components["schemas"]["SyncPlaylistItem"][];
            playlists: components["schemas"]["SyncPlaylist"][];
            recordings: components["schemas"]["SyncRecording"][];
            savedAlbums: components["schemas"]["SyncSavedAlbum"][];
            settings?: components["schemas"]["SyncSettings"] | null;
            tombstones: components["schemas"]["SyncTombstone"][];
            trackFeatures?: components["schemas"]["SyncTrackFeatures"][];
            tracks: components["schemas"]["SyncTrack"][];
            wavePresets?: components["schemas"]["SyncWavePreset"][];
        };
        SyncPlaylist: {
            /** @description Совместный плейлист. */
            collaborative?: boolean;
            /** Format: date-time */
            createdAt: string;
            description: string;
            /** Format: uuid */
            folderId?: string | null;
            friendsCanSee?: boolean;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["PlaylistKind"];
            /** @description Участники совместного (без владельца): имена и аватары для «кто добавил». */
            members?: components["schemas"]["PersonRef"][];
            owner?: components["schemas"]["PersonRef"] | null;
            pinned: boolean;
            /** Format: double */
            position?: number;
            /** Format: int64 */
            rev: number;
            title: string;
            /** Format: date-time */
            updatedAt: string;
        };
        /** @description Папка плейлистов. */
        SyncPlaylistFolder: {
            collapsed: boolean;
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            id: string;
            /** Format: double */
            position: number;
            /** Format: int64 */
            rev: number;
            title: string;
            /** Format: date-time */
            updatedAt: string;
        };
        SyncPlaylistItem: {
            /** Format: date-time */
            addedAt: string;
            /**
             * Format: uuid
             * @description Кто добавил (в совместном плейлисте).
             */
            addedBy?: string | null;
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
        /** @description Настройки в синхронизации приложений: одна строка, `rev` — как у строк медиатеки. */
        SyncSettings: components["schemas"]["UserSettings"] & {
            /** Format: int64 */
            rev: number;
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
        /** @description Признаки звучания записи из анализа: по ним волна подбирает треки без сети. */
        SyncTrackFeatures: {
            /** Format: float */
            approachability: number | null;
            /** Format: float */
            bpm: number | null;
            /** Format: float */
            danceability: number | null;
            /** Format: float */
            energy: number | null;
            keyCamelot: string | null;
            language: string | null;
            /** Format: float */
            party: number | null;
            /** Format: uuid */
            recordingId: string;
            /** Format: float */
            relaxed: number | null;
            /** Format: int64 */
            rev: number;
            /** Format: float */
            valence: number | null;
            /** Format: float */
            voice: number | null;
        };
        /** @description Сохранённое сочетание настроек волны. */
        SyncWavePreset: {
            /** Format: date-time */
            createdAt: string;
            icon: string;
            /** Format: uuid */
            id: string;
            name: string;
            /** Format: double */
            position: number;
            /** Format: int64 */
            rev: number;
            settings: components["schemas"]["WaveSettings"];
        };
        /** @description «Меньше такого»: приглушённое направление звучания. Переживает ночной пересчёт граней. */
        TasteDamper: {
            /** Format: date-time */
            createdAt: string;
            /** @description До трёх треков, по которым понятно, о чём речь. */
            examples: components["schemas"]["Track"][];
            /** Format: uuid */
            id: string;
            label: string;
            /**
             * Format: float
             * @description Сила, 0..1: первое нажатие — 0,5, каждое следующее — сильнее, до 1.
             */
            strength: number;
            /** Format: date-time */
            updatedAt: string;
        };
        /** @description Грань вкуса глазами слушателя. */
        TasteFacet: {
            /** @description Главные артисты грани (до трёх). */
            artists: components["schemas"]["ArtistSummary"][];
            /**
             * Format: float
             * @description Насколько грань приглушена «меньше такого», 0..1.
             */
            damped: number;
            /**
             * Format: uuid
             * @description «Меньше такого», которое её приглушает: отмена — `DELETE /api/taste/dampers/{id}`.
             */
            damperId: string | null;
            /** @description Треки-примеры (до шести), самые характерные первыми. */
            examples: components["schemas"]["Track"][];
            /** Format: uuid */
            id: string;
            /** @description «Трэп · Lowborn, Mzlff» */
            label: string;
            /** @description Когда эту грань слушают заметно чаще остальных: `morning`, `day`, `evening`, `night`. */
            peak: string | null;
            /**
             * Format: float
             * @description Доля во вкусе, 0..1, с учётом «меньше такого»; сумма по граням — 1.
             */
            share: number;
            /** Format: int32 */
            tracks: number;
            /** Format: date-time */
            updatedAt: string;
        };
        /** @description Из чего волна знает вкус. */
        TasteKnowledge: {
            /**
             * Format: int64
             * @description Из лайкнутого и дослушанного разобрано по звучанию.
             */
            analyzed: number;
            /** Format: int64 */
            blocked: number;
            /** Format: int64 */
            dislikes: number;
            /** Format: int64 */
            likes: number;
            /**
             * Format: int64
             * @description Треков, которые слушатель дослушивал.
             */
            listened: number;
            /**
             * Format: int64
             * @description Реакций в волне (лайк, пропуск, «не сейчас»…) за 30 дней.
             */
            reactions: number;
            /**
             * Format: date-time
             * @description Последний сброс вкуса: прослушивания до него грани не строят.
             */
            resetAt: string | null;
            /**
             * Format: date-time
             * @description Когда грани собирались в последний раз.
             */
            updatedAt: string | null;
        };
        /**
         * @description Совпадение вкусов. Честная метрика из двух частей (каждая 0…1):
         *     - `artists` — пересечение долей прослушиваний по артистам за полгода: сумма по общим артистам
         *       меньшей из двух долей (1 — слушаете одних и тех же в тех же пропорциях);
         *     - `sound` — насколько близки грани вкуса по звучанию (взвешенная лучшая пара граней, косинус
         *       приведён к шкале, где «случайные треки» — около нуля).
         *
         *     `percent` — среднее двух частей (или одна, если второй нет).
         */
        TasteMatch: {
            /** Format: float */
            artists: number | null;
            /** Format: int32 */
            percent: number;
            /** Format: float */
            sound: number | null;
        };
        /** @description `GET /api/taste`. */
        TasteOverview: {
            dampers: components["schemas"]["TasteDamper"][];
            facets: components["schemas"]["TasteFacet"][];
            knows: components["schemas"]["TasteKnowledge"];
            stats: components["schemas"]["ListeningStats"];
        };
        TastePayload: {
            /** @description Собрать «Открытия недели» заново, даже если им нет недели. */
            refreshDiscovery?: boolean;
            /** Format: uuid */
            userId: string;
        };
        /** @description `POST /api/taste/reset`: подтверждение обязательно — сброс не отменить. */
        TasteReset: {
            confirm: boolean;
        };
        TopArtist: {
            artist: components["schemas"]["ArtistSummary"];
            /** Format: int64 */
            minutes: number;
            /** Format: int64 */
            plays: number;
        };
        TopGenre: {
            /** @description Жанр Deezer как есть («Rap/Hip Hop») — английский интерфейс показывает его. */
            id: string;
            /** @description По-русски («Рэп и хип-хоп»). */
            name: string;
            /**
             * Format: float
             * @description Доля времени прослушивания, 0..1.
             */
            share: number;
        };
        /** @description Трек каталога — позиция в альбоме. Одна запись (ISRC) может быть у нескольких треков. */
        Track: {
            album: components["schemas"]["AlbumRef"] | null;
            /** @description Сначала основные артисты, затем приглашённые. */
            artists: components["schemas"]["TrackArtist"][];
            audioStatus: components["schemas"]["AudioStatus"];
            /**
             * Format: uuid
             * @description У explicit-трека — чистая версия той же песни в каталоге: её играет фильтр «Без нецензурного».
             */
            cleanId?: string | null;
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
        /** @description Ответ `GET /api/tracks/{id}/analysis`. Списки — пустые, а не `null`, если данных нет. */
        TrackAnalysis: {
            beatsMs: number[];
            /** Format: float */
            bpm: number | null;
            /**
             * Format: float
             * @description Уверенность модели в сетке долей, 0…1.
             */
            bpmConfidence: number | null;
            cues: components["schemas"]["AnalysisCues"] | null;
            downbeatsMs: number[];
            /** Format: int32 */
            durationMs: number | null;
            /** Format: float */
            energy: number | null;
            /** @description Энергия по секундам, 0…1000. */
            energyCurve: number[];
            /**
             * Format: uuid
             * @description Файл, к которому привязана сетка: не совпал с `fileId` потока — сетка от другого файла.
             */
            fileId: string | null;
            /** Format: int32 */
            introEndMs: number | null;
            /** @description 8A, 11B … */
            keyCamelot: string | null;
            /** Format: float */
            keyConfidence: number | null;
            /** @description Начала фраз по 16 тактов (подмножество `phrasesMs`). */
            majorPhrasesMs: number[];
            /** Format: int32 */
            outroStartMs: number | null;
            /** @description Начала фраз по 8 тактов. */
            phrasesMs: number[];
            /** Format: uuid */
            trackId: string;
            /** @description Участки с вокалом: `[начало, конец]` в мс. */
            vocalSegments: number[][];
        };
        TrackArtist: {
            /** Format: uuid */
            id: string;
            name: string;
            role: components["schemas"]["ArtistRole"];
        };
        /** @description Ответ `GET /api/tracks/{id}/credits`. */
        TrackCredits: {
            label: string | null;
            /** @description Продюсеры и звукорежиссёры. */
            production: components["schemas"]["CreditPerson"][];
            /** @description Страницы людей ещё ищутся: через несколько секунд у части имён появятся ссылки. */
            resolving: boolean;
            /** Format: uuid */
            trackId: string;
            /** @description Авторы музыки и слов. */
            writing: components["schemas"]["CreditPerson"][];
            /** Format: int32 */
            year: number | null;
        };
        /** @description Ответ `GET /api/tracks/{id}/hook`: что играть ~15 секунд перед включением, не трогая очередь. */
        TrackHook: {
            from: components["schemas"]["HookFrom"];
            /**
             * Format: int64
             * @description Сколько звучит отрывок, мс.
             */
            lengthMs: number;
            /**
             * @description Звука на сервере нет — подписанная ссылка на MP3-превью Deezer (живёт недолго, не хранить).
             *     `null` вместе с `ready: false` — послушать нечего.
             */
            previewUrl: string | null;
            /** @description Звук трека на сервере: отрывок — из него, с `startMs` (ссылку плеер берёт как обычно). */
            ready: boolean;
            /**
             * Format: int64
             * @description Начало отрывка от начала звука, мс. У превью — 0: Deezer сам режет его с припева.
             */
            startMs: number;
            /** Format: uuid */
            trackId: string;
        };
        TrackList: {
            items: components["schemas"]["Track"][];
        };
        TrackLookup: {
            /** @description До 500 id; неизвестные пропускаются. */
            ids: string[];
        };
        /** @description Ответ `GET /api/tracks/{id}/lyrics`. */
        TrackLyrics: {
            kind: components["schemas"]["LyricsKind"];
            /** @description Строки по возрастанию времени — только у `synced`, иначе пусто. */
            lines: components["schemas"]["LyricsLine"][];
            /** @description Текст целиком — у `plain` (и у `synced`, если источник его дал). */
            plain: string | null;
            /** @description Откуда текст: `lrclib`. */
            source: string | null;
            /** Format: uuid */
            trackId: string;
        };
        /** @description Свежая ссылка на 30-секундное превью Deezer. */
        TrackPreview: {
            /** @description `null` — у трека нет превью или его нет у Deezer. */
            url: string | null;
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
        /** @description Одна версия песни: по одной на вид. */
        TrackVersion: {
            kind: components["schemas"]["VersionKind"];
            track: components["schemas"]["Track"];
        };
        /** @description Версии песни трека. В `items` — и сам трек; версий нет — в `items` только он. */
        TrackVersions: {
            /** @description По порядку: оригинал, slowed, super slowed, sped up. */
            items: components["schemas"]["TrackVersion"][];
            /** @description Вид этого трека. */
            kind: components["schemas"]["VersionKind"];
            preferred: components["schemas"]["VersionKind"] | null;
            /** Format: uuid */
            trackId: string;
        };
        /**
         * @description Задание компьютеру: набор и описание обучения (`dalywave-taste`, `wire::Package`). Отложенной части
         *     в наборе нет — её проверяет сервер.
         */
        TrainClaim: {
            package: Record<string, never>;
            /** Format: int64 */
            rows: number;
            /** @description `global` или `personal`. */
            scope: components["schemas"]["TrainScope"];
            /** Format: uuid */
            taskId: string;
        };
        TrainClaimBody: {
            scope?: components["schemas"]["TrainScope"];
        };
        /** @description Обучение на компьютере не удалось. */
        TrainFailure: {
            message: string;
        };
        /** @description Что сервер решил о присланных весах. */
        TrainOutcome: {
            /** @description Собирает ли поток теперь эта версия. */
            active: boolean;
            note: string;
            version: string;
        };
        /**
         * @description Чью сеть учить на этом компьютере.
         * @enum {string}
         */
        TrainScope: "auto" | "global" | "personal";
        /** @description Веса и как шло обучение (`dalywave-taste`: `wire::WireParams`, `TrainReport`). */
        TrainSubmit: {
            params: Record<string, never>;
            report: Record<string, never>;
        };
        /** @description Задание «обучить сеть вкуса» компьютеру с приложением. */
        TrainTaskView: {
            deviceName: string;
            /** Format: date-time */
            finishedAt: string | null;
            /** Format: uuid */
            id: string;
            /** Format: date-time */
            leasedAt: string;
            /** @description Решение сервера или ошибка компьютера. */
            message: string | null;
            /**
             * Format: int64
             * @description Строк обучающей части.
             */
            rows: number;
            /** @description `leased`, `done`, `failed`, `expired`. */
            status: string;
            /** @description Своя сеть слушателя; `null` — общая. */
            userName: string | null;
            /** @description Версия из присланных весов. */
            version: string | null;
        };
        /**
         * @description Режим переходов между треками.
         * @enum {string}
         */
        TransitionMode: "gapless" | "smooth" | "smart" | "dj";
        /**
         * @description Ответ в формате tauri-plugin-updater. Имена полей задаёт плагин (`pub_date`), поэтому здесь
         *     не camelCase.
         */
        UpdateManifest: {
            /** @description Что нового */
            notes?: string | null;
            /** Format: date-time */
            pub_date: string;
            /** @description Содержимое `.sig` из сборки: плагин проверяет файл ключом из `tauri.conf.json` */
            signature: string;
            /** @description Подписанная ссылка на файл обновления, живёт 6 часов */
            url: string;
            /** @description SemVer новой версии */
            version: string;
        };
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
        UserSettings: {
            /** @description Подхват музыки на другом устройстве (`connect.signal` → `connect.handoff`). */
            handoff?: components["schemas"]["HandoffMode"];
            /**
             * @description «Без нецензурного»: explicit-треки не играют в волне, миксах и автопродолжении, в списках — серым;
             *     если в каталоге есть чистая версия, играет она.
             */
            hideExplicit: boolean;
            playlistSort: components["schemas"]["PlaylistSort"];
            qualityCellular: components["schemas"]["CellularQuality"];
            qualityDownload: components["schemas"]["DownloadQuality"];
            qualityWifi: components["schemas"]["WifiQuality"];
            /**
             * Format: date-time
             * @description `null` — ни разу не меняли (всё по умолчанию).
             */
            updatedAt: string | null;
        };
        /** @description Изменить часть настроек; не названные поля остаются как были. */
        UserSettingsPatch: {
            handoff?: components["schemas"]["HandoffMode"] | null;
            hideExplicit?: boolean | null;
            playlistSort?: components["schemas"]["PlaylistSort"] | null;
            qualityCellular?: components["schemas"]["CellularQuality"] | null;
            qualityDownload?: components["schemas"]["DownloadQuality"] | null;
            qualityWifi?: components["schemas"]["WifiQuality"] | null;
        };
        /** @description Исполнитель, у которого человек выбрал не оригинал. */
        VersionChoice: {
            /** Format: uuid */
            artistId: string;
            kind: components["schemas"]["VersionKind"];
        };
        /** @description Выбор версии: в следующий раз у этого исполнителя играет та же. */
        VersionChoiceBody: {
            kind: components["schemas"]["VersionKind"];
        };
        VersionChoices: {
            items: components["schemas"]["VersionChoice"][];
        };
        /**
         * @description Вид версии по названию релиза: «(Slowed + Reverb)» — slowed, «(Ultra Slowed)» — super slowed,
         *     «(Nightcore)» — sped up. Ремиксы, 8D и концертные записи версиями не считаются.
         * @enum {string}
         */
        VersionKind: "original" | "slowed" | "super_slowed" | "sped_up";
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
        /** @description Заготовки волны под одно сочетание настроек. */
        WavePoolView: {
            /** @description `занятие:характер:настроение:язык[:темп]`. */
            combo: string;
            /**
             * Format: date-time
             * @description Последний подбор.
             */
            pickedAt: string;
            /**
             * Format: int64
             * @description Из них уже на сервере.
             */
            ready: number;
            settings: components["schemas"]["WaveSettings"];
            /** Format: int64 */
            tracks: number;
        };
        /**
         * @description Готова ли волна к запуску.
         * @enum {string}
         */
        WavePrepState: "preparing" | "ready";
        /** @description Сохранённое сочетание настроек волны: запускает волну в одно нажатие. */
        WavePreset: {
            /** Format: date-time */
            createdAt: string;
            /** @description Имя иконки Phosphor без префикса: `laptop`, `moon-stars`. */
            icon: string;
            /** Format: uuid */
            id: string;
            name: string;
            /**
             * Format: double
             * @description Порядок карточек — по возрастанию.
             */
            position: number;
            /** @description Без затравки: радио не сохраняется. */
            settings: components["schemas"]["WaveSettings"];
        };
        /** @description Сочетания по порядку карточек. */
        WavePresetList: {
            items: components["schemas"]["WavePreset"][];
        };
        /** @description Новый порядок: названные сочетания — первыми в этом порядке, остальные — после, как были. */
        WavePresetOrder: {
            ids: string[];
        };
        /** @description Правка сочетания: переданные поля меняются. */
        WavePresetPatch: {
            icon?: string | null;
            name?: string | null;
            settings?: components["schemas"]["WaveSettings"] | null;
        };
        /** @description Затравка: `id` — для трека, артиста, альбома и плейлиста, `text` — для описания словами и года. */
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
             * @description … и сколько нужно, чтобы начать.
             */
            need: number;
            /** @description Начало волны в порядке игры: подобрано под настройки, готовое — первым. Дальше — `next`. */
            queue: components["schemas"]["WaveItem"][];
            /**
             * Format: int32
             * @description Сколько треков начала уже на сервере …
             */
            ready: number;
            /**
             * Format: int32
             * @description Сколько треков уже выдано.
             */
            served: number;
            settings: components["schemas"]["WaveSettings"];
            /** Format: date-time */
            startedAt: string;
            /** @description `preparing` — первые треки под настройки ещё скачиваются; `ready` — очередь можно играть. */
            state: components["schemas"]["WavePrepState"];
            /** @description Подпись: «Моя волна», «Радио: Lowborn», «По запросу «…»». */
            title: string;
        };
        /** @description Сессия волны или радио слушателя и чем кончались её показы. */
        WaveSessionView: {
            /** Format: int64 */
            completed: number;
            /** Format: uuid */
            id: string;
            /** Format: date-time */
            lastActiveAt: string;
            /**
             * Format: int64
             * @description Исходы показов: лайки, дослушано, пропуски (с «не нравится»).
             */
            likes: number;
            /**
             * Format: int64
             * @description Сколько треков выдано.
             */
            served: number;
            settings: components["schemas"]["WaveSettings"];
            /** Format: int64 */
            skips: number;
            /** Format: date-time */
            startedAt: string;
            /** @description «Моя волна», «Радио: Lowborn». */
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
        WebPushKeys: {
            /** @description Секрет аутентификации, base64url. */
            auth: string;
            /** @description Открытый ключ браузера P-256, base64url. */
            p256dh: string;
        };
        /** @description Подписка браузера — `PushSubscription.toJSON()`. */
        WebPushSubscription: {
            endpoint: string;
            keys: components["schemas"]["WebPushKeys"];
        };
        /**
         * @description О чём строка объяснения — для значка.
         * @enum {string}
         */
        WhyKind: "artist" | "recent" | "facet" | "familiar" | "new" | "moment" | "seed" | "release" | "explore" | "learned";
        WhyLine: {
            kind: components["schemas"]["WhyKind"];
            /** @description Готовый текст по-русски: «Похоже на Rushex, которого ты часто слушаешь». */
            text: string;
        };
        /** @description «Почему этот трек» — понятными словами, по тому, что на самом деле повлияло на выбор. */
        WhyTrack: {
            /** @description Только администратору: оценки и сходства, версия сети, грань — как в разделе «Рекомендации». */
            details: Record<string, never> | null;
            /** @description Главное — первым. */
            lines: components["schemas"]["WhyLine"][];
            track: components["schemas"]["Track"];
        };
        /**
         * @description Качество по Wi-Fi: «Высокое» — lossless, если он есть у записи, иначе лучший файл; «Обычное» — Opus
         *     130–160 кбит/с.
         * @enum {string}
         */
        WifiQuality: "high" | "normal";
        /**
         * @description «Неверная запись»: что не так с тем, что играет.
         * @enum {string}
         */
        WrongRecordingReason: "wrong_song" | "wrong_version" | "bad_quality" | "cut";
        WrongRecordingReport: {
            /** @description Необязательный комментарий, до 500 символов. */
            comment?: string | null;
            reason: components["schemas"]["WrongRecordingReason"];
        };
        /** @description Жалоба принята: спорная запись в админке и новое скачивание без этой записи. */
        WrongRecordingResult: {
            /**
             * Format: uuid
             * @description Новое скачивание; нет — у записи уже идёт скачивание или её не скачать заново.
             */
            acquisitionId: string | null;
            /**
             * Format: uuid
             * @description Скачивание со статусом `disputed` — в разделе «Спорные».
             */
            disputeId: string;
        };
        /**
         * @description Перенос из «Яндекс Музыки». Ссылка — на плейлист (`music.yandex.ru/users/<логин>/playlists/<номер>`,
         *     `music.yandex.ru/playlists/<id>`) или на «Мне нравится» открытого профиля (`…/users/<логин>/tracks`).
         *     Ключ входа (OAuth-токен «Яндекс Музыки») нужен, если Яндекс не отдаёт данные без входа; с ним и без
         *     ссылки переносятся «Мне нравится» и все плейлисты аккаунта. Ключ используется только в этом
         *     запросе: не сохраняется и не пишется в журналы.
         */
        YandexImportRequest: {
            token?: string | null;
            url?: string | null;
        };
        /** @description Плейлист YouTube или YouTube Music по ссылке (`…?list=PL…`). */
        YoutubeImportRequest: {
            url: string;
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
                /** @description Только идущие: в очереди, поиск, скачивание, проверка, обработка. */
                active?: boolean;
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
    one: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id скачивания */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Скачивание: трек, кандидаты с разбором оценки, попытки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminAcquisition"];
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
                /** @description Вид события или его группа: `auth.sign_in` — ровно он, `auth.` — все входы и выходы. */
                kind?: string;
                /** @description Только события этого человека. */
                actor?: string;
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
    imports: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Импорты всех людей, новые первыми (до 100) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminImportList"];
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
            /** @description Параллельность 1–16, пауза 0–3 600 000 мс, окно — минуты 0–1439, начало и конец заданы оба и не совпадают */
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
            query?: {
                /** @description Слушатель; по умолчанию — тот, кто смотрит. */
                userId?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Вкус слушателя (грани, показы, сессии, заготовки, миксы) и общий индекс каталога: анализ, находки разведчика, сервис анализа, настройки */
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
            /** @description Нет такого слушателя */
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
            /** @description Нет такого слушателя */
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
    recordings: {
        parameters: {
            query?: {
                /** @description Состояние аудио; по умолчанию — все, у которых есть файл или запрос на скачивание. */
                status?: components["schemas"]["AudioStatus"];
                /** @description Источник основного файла: `youtube`, `youtube_music`, `soundcloud`, `bandcamp`, `upload`, `other`. */
                source?: string;
                /** @description Только непроверенные отпечатком (`false`) или только проверенные (`true`). */
                verified?: boolean;
                /** @description Название трека или имя артиста. */
                q?: string;
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
            /** @description Записи с файлом или запросом на скачивание, недавно менявшиеся первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminRecordingPage"];
                };
            };
            /** @description Неверный cursor, limit или источник */
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
    reacquire: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id записи */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReplaceAudio"];
            };
        };
        responses: {
            /** @description Новое скачивание вне очереди. Со ссылкой файл заменит основной, без неё — заменит, только если окажется лучше */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminAcquisition"];
                };
            };
            /** @description Неверная ссылка или у записи нет трека Deezer */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такой записи */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Скачивание записи уже идёт (`acquisition_active`) */
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
    event_settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description «Итоги года» как событие: стиль, режим, что открыто сейчас */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RewindEvent"];
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
    update_event: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RewindEventPatch"];
            };
        };
        responses: {
            /** @description Событие после изменения */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RewindEvent"];
                };
            };
            /** @description Год — от 2000 до текущего */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
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
    settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Настройки сервера */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ServerSettings"];
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
    update_settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ServerSettingsPatch"];
            };
        };
        responses: {
            /** @description Настройки после изменения */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ServerSettings"];
                };
            };
            /** @description Часовой пояс — от −720 до 840 минут */
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
    check_download: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SourceCheck"];
            };
        };
        responses: {
            /** @description Трек поставлен на скачивание вне очереди (или уже качается) — за ним видно, как работают источники */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AdminAcquisition"];
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
            /** @description Скачивать нечего: у всех треков уже есть файл (`nothing_to_check`) */
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
    storage: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Занятое место по видам, кодекам и источникам, тяжёлые артисты, прирост */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["StorageOverview"];
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
    preview: {
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
            /** @description Свежая подписанная ссылка на превью Deezer (живёт недолго) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackPreview"];
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
    artist_about: {
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
            /** @description Начало статьи Википедии и факты: годы на сцене, откуда, жанры. `about: null` — статьи нет */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ArtistInfo"];
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
            /** @description Википедия не ответила */
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
    app_pair: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PairRequest"];
            };
        };
        responses: {
            /** @description Сессия часов: отдельное устройство в «Доступе», отзывается вместе с iPhone. Токен показывается один раз */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AppToken"];
                };
            };
            /** @description Платформа — не сопряжённое устройство */
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
            /** @description Просит не приложение (сайт) или само сопряжённое устройство */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Больше 10 запросов в минуту с одного адреса */
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
    list_charts: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Чарты: Россия, мир, жанры — с обложкой первого места */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ChartList"];
                };
            };
        };
    };
    chart: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description `ru`, `world` или `genre-…` из списка чартов */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Треки по местам, альбомы и артисты по трекам чарта. `updating` — чарт собирается, стоит спросить ещё раз через несколько секунд */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ChartDetail"];
                };
            };
            /** @description Нет такого чарта */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Deezer не ответил, а собранного чарта ещё нет */
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
    describe_preview: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DescribeRequest"];
            };
        };
        responses: {
            /** @description Предпросмотр: что понято, название, описание и треки в плавном порядке. Первые треки, которых нет на сервере, уже качаются. Сервис анализа не ответил — подбор без поиска по звучанию (по жанрам, ориентирам и вкусу) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DescribePreview"];
                };
            };
            /** @description Пустое или длиннее 300 символов описание */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком часто: не больше 30 сборок за 10 минут */
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
    describe_more: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DescribeMore"];
            };
        };
        responses: {
            /** @description Ещё треки в том же духе: без показанных, начало подстроено к `afterTrackId` */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WaveItems"];
                };
            };
            /** @description count вне 1–30 или понятое не разобрать */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком часто */
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
    save_described: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SaveDescribed"];
            };
        };
        responses: {
            /** @description Обычный плейлист человека; помнит понятое — для «Ещё в том же духе» и обновления раз в неделю. Чего нет на сервере, ставится на скачивание */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Пустое название, нет известных треков, понятое не разобрать */
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
    devices: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Устройства (активное первым) и последний снимок с позицией на сейчас */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConnectOverview"];
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
    list_downloads: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Что закреплено на этом устройстве и сколько из этого на сервере */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DeviceDownloadList"];
                };
            };
            /** @description Вход не токеном приложения */
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
    add_download: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewDeviceDownload"];
            };
        };
        responses: {
            /** @description Закреплено (повторный запрос возвращает то же); недостающее на сервере ставится на скачивание */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DeviceDownload"];
                };
            };
            /** @description Нет refId или вход не токеном приложения */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого трека, альбома, плейлиста или артиста */
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
    remove_download: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id закреплённого */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Откреплено (или уже не было) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Вход не токеном приложения */
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
    files: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["FileLinksRequest"];
            };
        };
        responses: {
            /** @description Подписанные ссылки на файлы (6 часов) с размером и хешем; чего нет на сервере — `pending`, оно ставится на скачивание */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["FileLinks"];
                };
            };
            /** @description Больше 200 треков или вход не токеном приложения */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Хранилище не подписало ссылку */
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
    rebuild_set: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DeviceSetRequest"];
            };
        };
        responses: {
            /** @description Набор волны устройства пересобран; строки придут синхронизацией (`deviceSets`) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DeviceSet"];
                };
            };
            /** @description Вход не токеном приложения */
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
    remote_presence: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConnectHello"];
            };
        };
        responses: {
            /** @description Пульт отметился (в сети ещё 15 с). Устройства и снимок активного — без очереди, истории и формы волны */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConnectOverview"];
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
    remote_command: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConnectSend"];
            };
        };
        responses: {
            /** @description Подтверждение устройства: `ok`, или `error` — `offline` (не в сети), `timeout` (не ответило за 4 с), `failed`, `unsupported` */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConnectAck"];
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
    remote_transfer: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ConnectTransfer"];
            };
        };
        responses: {
            /** @description Перенос: `ok`, или `error` — `offline`, `unsupported` (на часы звук не переносится) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ConnectAck"];
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
    list_get_friends: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Друзья (слушающие сейчас — первыми) и запросы */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["FriendList"];
                };
            };
        };
    };
    invite: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Своя ссылка «Добавить в друзья» */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InviteLink"];
                };
            };
        };
    };
    reset_invite: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Новая ссылка; прежняя больше не работает */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InviteLink"];
                };
            };
        };
    };
    invite_preview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Код из ссылки */
                code: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Кто зовёт в друзья и кем уже приходится */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonCard"];
                };
            };
            /** @description Ссылка больше не работает (`invite_expired`) */
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
    use_invite: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Код из ссылки */
                code: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Теперь друзья: ссылку дал сам человек */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PersonCard"];
                };
            };
            /** @description Ссылка больше не работает (`invite_expired`) */
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
    people: {
        parameters: {
            query?: {
                /** @description Часть имени; пусто — все, у кого есть доступ к сервису. */
                q?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Люди сервиса (с доступом) по имени и кем приходятся */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PeopleList"];
                };
            };
        };
    };
    get_privacy: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Показывать ли друзьям, что я слушаю, и идёт ли приватная сессия */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Privacy"];
                };
            };
        };
    };
    update_privacy: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PrivacyPatch"];
            };
        };
        responses: {
            /** @description Приватность после изменения */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Privacy"];
                };
            };
        };
    };
    request: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewFriendRequest"];
            };
        };
        responses: {
            /** @description Запрос отправлен (`outgoing`) или встречный уже был — друзья (`friend`) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["FriendRequestResult"];
                };
            };
            /** @description Себе запрос не отправить */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого человека */
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
    cancel_request: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Кому отправлен запрос */
                userId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Запрос отменён (если был) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    accept: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Кто прислал запрос */
                userId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Теперь друзья */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["FriendRequestResult"];
                };
            };
            /** @description Запроса нет */
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
    decline: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Кто прислал запрос */
                userId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Запрос отклонён: отправитель об этом не узнает, повтор не тревожит */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    profile: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Человек */
                userId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Страница друга: что слушает, открытые плейлисты, топ за месяц, совпадение вкусов. Не другу — только имя и кем приходится */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["FriendProfile"];
                };
            };
            /** @description Нет такого человека */
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
    unfriend: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Друг */
                userId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Больше не друзья (с обеих сторон) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
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
                /** @description albums, artists или playlists (своя обложка плейлиста) */
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
    list_get_imports: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Свои импорты, новые первыми (до 50) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportList"];
                };
            };
        };
    };
    start_files: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["FilesImportRequest"];
            };
        };
        responses: {
            /** @description Импорт файлов открыт: дальше файлы по одному, затем finish */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
        };
    };
    import_list: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ListImportRequest"];
            };
        };
        responses: {
            /** @description Список разобран, поиск треков идёт в фоне */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
            /** @description В тексте не нашлось ни одного трека */
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
    upload_spotify: {
        parameters: {
            query?: {
                /** @description Имя файла, как у человека на диске. */
                name?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Архив `my_spotify_data.zip` (или один JSON из него), до 500 МБ */
        requestBody: {
            content: {
                "application/zip": number[];
            };
        };
        responses: {
            /** @description Архив разобран, сопоставление идёт в фоне */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
            /** @description Не архив Spotify или он повреждён */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Импорт архива уже идёт */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Больше 500 МБ */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    import_yandex: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["YandexImportRequest"];
            };
        };
        responses: {
            /** @description Треки собраны, сопоставление идёт в фоне */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
            /** @description Нет ни ссылки, ни ключа входа, или в плейлисте нет треков */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Ссылка не «Яндекс Музыки» (`yandex_link`), ключ не похож на ключ (`yandex_token`), нужен ключ входа (`yandex_login_required`), ключ не подошёл (`yandex_token_rejected`), плейлиста нет или он скрыт (`yandex_not_found`), Яндекс не отвечает стране сервера (`yandex_region`) */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description «Яндекс Музыка» не ответила */
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
    import_youtube: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["YoutubeImportRequest"];
            };
        };
        responses: {
            /** @description Треки плейлиста собраны, сопоставление идёт в фоне */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
            /** @description В плейлисте нет треков */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Не ссылка на плейлист (`youtube_link`) или плейлист закрыт (`youtube_not_found`) */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description YouTube Music недоступен */
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
    upload_takeout: {
        parameters: {
            query?: {
                /** @description Имя файла, как у человека на диске. */
                name?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** @description Архив Google Takeout «YouTube и YouTube Music», до 500 МБ */
        requestBody: {
            content: {
                "application/zip": number[];
            };
        };
        responses: {
            /** @description Архив разобран, сопоставление идёт в фоне */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
            /** @description Не архив Takeout или в нём нет музыки */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Больше 500 МБ */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    one_get_imports_id_: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id импорта */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Импорт: шаг, прогресс, итог */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
            /** @description Нет такого импорта */
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
                /** @description id импорта */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Идущий импорт остановлен, законченный — убран из истории (медиатека не меняется) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого импорта */
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
    upload_file: {
        parameters: {
            query?: {
                /** @description Имя файла, как у человека на диске. */
                name?: string;
            };
            header?: never;
            path: {
                /** @description id импорта файлов */
                id: string;
            };
            cookie?: never;
        };
        /** @description Аудиофайл: FLAC, ALAC/M4A, MP3, Opus, Ogg, WAV, AIFF — до 1 ГБ */
        requestBody: {
            content: {
                "application/octet-stream": number[];
            };
        };
        responses: {
            /** @description Файл принят и ждёт обработки */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportItem"];
                };
            };
            /** @description Не аудиофайл или формат не поддерживается */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Импорт уже закрыт для новых файлов */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Файл больше 1 ГБ */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    finish_files: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id импорта файлов */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Файлы загружены; импорт закончится, когда все обработаются */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportSummary"];
                };
            };
            /** @description Нет такого импорта */
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
    items: {
        parameters: {
            query?: {
                /** @description Только позиции в этом состоянии; без него — все. */
                status?: components["schemas"]["ImportItemStatus"];
                cursor?: string;
                /** @description 1–200, по умолчанию 50. */
                limit?: number;
            };
            header?: never;
            path: {
                /** @description id импорта */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Позиции по порядку источника */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportItemPage"];
                };
            };
            /** @description Нет такого импорта */
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
    fix_item: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id импорта */
                id: string;
                /** @description id позиции */
                itemId: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ImportItemFix"];
            };
        };
        responses: {
            /** @description Позиция исправлена; если импорт уже разложен — найденное сразу в медиатеке */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ImportItem"];
                };
            };
            /** @description Не указан трек или он не подходит к позиции */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого импорта, позиции или трека */
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
    jam_overview: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Открытая сессия «Слушать вместе» или `jam: null` */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamOverview"];
                };
            };
        };
    };
    jam_start: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сессия уже открыта — она же */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Jam"];
                };
            };
            /** @description Новая сессия: код, ссылка для гостей */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Jam"];
                };
            };
        };
    };
    jam_end: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сессия закончена: токены гостей больше не действуют */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    jam_update: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["JamPatch"];
            };
        };
        responses: {
            /** @description Сессия с новыми настройками */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Jam"];
                };
            };
            /** @description Открытой сессии нет (`jam_ended`) */
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
    jam_guest_view: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Страница гостя: что играет, очередь, участники, свои треки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamGuestView"];
                };
            };
            /** @description Нет токена гостя */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Сессия закончилась (`jam_ended`) или хозяин убрал гостя (`jam_removed`) */
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
    jam_leave: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Гость вышел: токен больше не действует */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    jam_add: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["JamAdd"];
            };
        };
        responses: {
            /** @description Трек в очереди (`added`) или ждёт одобрения хозяина (`pending`) */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamRequest"];
                };
            };
            /** @description Хозяин выключил добавление гостями */
            403: {
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
            /** @description `already_queued` — уже в очереди; `too_many_pending` — пять треков уже ждут; `nothing_playing` — у хозяина ничего не на связи; `explicit_off` — хозяин слушает без нецензурного, а трек explicit */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Больше 10 треков в минуту */
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
    jam_react: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["JamReact"];
            };
        };
        responses: {
            /** @description ❤ ушло всем в сессии */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Больше 30 реакций в минуту */
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
    jam_search: {
        parameters: {
            query: {
                /** @description Запрос, 1–100 символов. */
                q: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Треки каталога (до 20) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamSearchResults"];
                };
            };
            /** @description Больше 30 поисков в минуту */
            429: {
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
    jam_skip: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Голос учтён; набралось больше половины гостей на связи — трек пропущен */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamSkip"];
                };
            };
            /** @description Ничего не играет (`nothing_playing`) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком часто */
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
    jam_stream: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Текущий или следующий трек хозяина */
                trackId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Подписанная ссылка на аудио (45 минут) для «Слушать у себя» */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamStream"];
                };
            };
            /** @description Хозяин выключил «Слушать у себя» или трек не текущий и не следующий */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Файла трека ещё нет на сервере (`audio_unavailable`) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком часто */
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
    jam_guest_ws: {
        parameters: {
            query: {
                /** @description Токен гостя `dwj_…`. */
                token: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description WebSocket гостя: `jam.now`, `jam.changed`, `jam.reaction`, `pong` */
            101: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Неверный токен */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Сессия закончилась (`jam_ended`) */
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
    jam_remove_guest: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id гостя */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Гость убран: его токен больше не действует */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого гостя в открытой сессии */
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
    jam_invite: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Код из ссылки */
                code: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Кто приглашает и что играет */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamInvite"];
                };
            };
            /** @description Нет такой открытой сессии (`jam_ended`) */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком часто с одного адреса */
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
    jam_join: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Код из ссылки */
                code: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["JamJoin"];
            };
        };
        responses: {
            /** @description Гость в сессии: токен `dwj_…` (показывается один раз) */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["JamJoined"];
                };
            };
            /** @description Нет имени */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такой открытой сессии (`jam_ended`) */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком много входов с одного адреса */
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
    jam_approve: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека гостя */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Трек ушёл в очередь активного устройства */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Jam"];
                };
            };
            /** @description Нет такого ждущего трека */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Активного устройства нет на связи (`nothing_playing`) */
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
    jam_decline: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека гостя */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Трек отклонён */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Jam"];
                };
            };
            /** @description Нет такого ждущего трека */
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
    search_lyrics: {
        parameters: {
            query: {
                /** @description Строчка песни, от 3 символов. */
                q: string;
                /** @description До 50, по умолчанию 20. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Треки, в тексте которых есть строчка (среди уже взятых текстов), и строка, где она нашлась */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["LyricsSearchResults"];
                };
            };
            /** @description Строчка короче 3 символов */
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
    history: {
        parameters: {
            query?: {
                /** @description `nextCursor` из предыдущей страницы. */
                cursor?: string;
                /** @description Сколько вернуть: 1–200, по умолчанию 50. */
                limit?: number;
                /** @description Поиск по названию трека, артисту и альбому (без регистра и диакритики). */
                q?: string;
                /** @description Переход к дате: прослушивания раньше этого момента. С `cursor` не сочетается — курсор главнее. */
                before?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Прослушивания, новые первыми, с названием источника; история из архива Spotify (`imported`) — тоже. Убранное из истории не приходит */
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
    clear_history: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description История очищена: прослушивания больше не видны в истории, но вкус, миксы и «Итоги года» их по-прежнему учитывают */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    history_days: {
        parameters: {
            query?: {
                /** @description Часовой пояс слушателя: минуты к востоку от UTC (Москва — 180), от −840 до 840. */
                offsetMin?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Дни с прослушиваниями по часовому поясу слушателя, новые первыми — для календаря */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["HistoryDays"];
                };
            };
            /** @description offsetMin вне −840…840 */
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
    hide_play: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id прослушивания из истории */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Прослушивание убрано из истории */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого прослушивания в истории */
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
    list_get_notifications: {
        parameters: {
            query?: {
                /** @description `next` из предыдущей страницы. */
                before?: string;
                /** @description Сколько вернуть: 1–100, по умолчанию 30. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Уведомления, новые первыми, и сколько непрочитанных */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NotificationPage"];
                };
            };
            /** @description Неверный курсор или limit */
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
    push_info: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Каналы push на сервере и подписанные устройства человека */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PushInfo"];
                };
            };
        };
    };
    subscribe: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PushRegistration"];
            };
        };
        responses: {
            /** @description Устройство подписано (повтор с тем же токеном обновляет подписку) */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PushRegistered"];
                };
            };
            /** @description Нет токена или подписки браузера, канал на сервере выключен */
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
    unsubscribe: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PushUnregister"];
            };
        };
        responses: {
            /** @description Устройство больше не получает push (если подписки не было — тоже) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нужен token или endpoint */
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
    read: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NotificationReadRequest"];
            };
        };
        responses: {
            /** @description Отмечено; другие устройства получат `notification.read` */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NotificationsRead"];
                };
            };
        };
    };
    get_settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Какие уведомления присылать и тихие часы */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NotificationSettings"];
                };
            };
        };
    };
    update_settings_patch_notifications_settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NotificationSettingsPatch"];
            };
        };
        responses: {
            /** @description Настройки после правки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["NotificationSettings"];
                };
            };
            /** @description Неверное время тихих часов или часовой пояс */
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
    test: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Пробное уведомление: в центре уведомлений и push на подписанные устройства (и в тихие часы) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PushTest"];
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
    info: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Ссылка на оверлей; `link: null` — ещё не создана */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OverlayInfo"];
                };
            };
        };
    };
    disable: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Оверлей выключен: ссылка больше не работает */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    update: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["OverlayPatch"];
            };
        };
        responses: {
            /** @description Ссылка после правки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OverlayInfo"];
                };
            };
            /** @description Ссылки ещё нет */
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
    rotate: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Новая ссылка; прежняя перестала работать */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OverlayLink"];
                };
            };
        };
    };
    now: {
        parameters: {
            query: {
                /** @description Токен из ссылки (`dwo_…`). */
                k: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Что сейчас играет у владельца ссылки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["OverlayNow"];
                };
            };
            /** @description Ссылка неверна, сменена или выключена */
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
    stream_now: {
        parameters: {
            query: {
                /** @description Токен из ссылки (`dwo_…`). */
                k: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description SSE: событие `now` (OverlayNow) сразу и при каждой перемене; `gone` — ссылка больше не работает */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/event-stream": unknown;
                };
            };
            /** @description Ссылка неверна, сменена или выключена */
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
    create: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewPlaylistFolder"];
            };
        };
        responses: {
            /** @description Папка создана первой вне папок; названные плейлисты уже в ней */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistList"];
                };
            };
            /** @description Пустое или длинное название, папок уже 100 */
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
    remove_delete_playlist_folders_id_: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id папки */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Папка удалена, её плейлисты встали на её место */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistList"];
                };
            };
            /** @description Нет такой папки */
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
    update_patch_playlist_folders_id_: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id папки */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PlaylistFolderPatch"];
            };
        };
        responses: {
            /** @description Папка после правки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistFolder"];
                };
            };
            /** @description Пустое или длинное название */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такой папки */
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
    move_folder: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id папки */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["MovePlaylistFolder"];
            };
        };
        responses: {
            /** @description Папка на новом месте */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistList"];
                };
            };
            /** @description Соседа нет вне папок */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такой папки */
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
    list_get_playlists: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Плейлисты (закреплённые, затем недавно изменённые) и папки */
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
    create_post_playlists: {
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
    join_preview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Код из ссылки */
                code: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Куда зовут: плейлист, владелец, вошёл ли уже */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistInvite"];
                };
            };
            /** @description Ссылка больше не работает (`invite_expired`) */
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
    join: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Код из ссылки */
                code: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Человек — участник; плейлист у него в медиатеке */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Ссылка больше не работает (`invite_expired`) */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Участников уже слишком много */
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
    remove_delete_playlists_id_: {
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
            /** @description Плейлист удалён вместе с позициями (и у участников) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Участник совместного плейлиста — может только выйти */
            403: {
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
    update_patch_playlists_id_: {
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
    make_collaborative: {
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
            /** @description Плейлист совместный: друзья, которых позовут, смогут добавлять треки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Не свой плейлист */
            403: {
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
            /** @description Микс — сначала сохранить копию */
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
    stop_collaborative: {
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
            /** @description Плейлист снова только свой: участники его больше не видят, ссылка не работает */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Не свой плейлист */
            403: {
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
    set_cover: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
            };
            cookie?: never;
        };
        /** @description Картинка: JPEG, PNG или WebP, до 15 МБ. Обрезается по центру до квадрата */
        requestBody: {
            content: {
                "image/jpeg": number[];
            };
        };
        responses: {
            /** @description Плейлист со своей обложкой */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Это не картинка */
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
            /** @description Плейлист собирается сам (микс) */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Файл больше 15 МБ */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    remove_cover: {
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
            /** @description Своей обложки больше нет — снова мозаика */
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
            /** @description Плейлист собирается сам (микс) */
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
    described: {
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
            /** @description Плейлист собран по описанию: понятое и обновление раз в неделю */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DescribedPlaylist"];
                };
            };
            /** @description Нет такого плейлиста или он собран не по описанию (`not_described`) */
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
    update_described: {
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
                "application/json": components["schemas"]["DescribedPatch"];
            };
        };
        responses: {
            /** @description Обновление раз в неделю включено или выключено */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DescribedPlaylist"];
                };
            };
            /** @description Нет такого плейлиста или он собран не по описанию */
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
    extend_described: {
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
                "application/json": components["schemas"]["DescribedMore"];
            };
        };
        responses: {
            /** @description В конец плейлиста добавлены треки в том же духе (без тех, что уже есть) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DescribedAdded"];
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
            /** @description Нет такого плейлиста или он собран не по описанию */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком часто */
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
    duplicate: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста или микса */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["DuplicatePlaylist"];
            };
        };
        responses: {
            /** @description Копия — обычный плейлист с теми же треками, описанием и обложкой */
            201: {
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
    invite_get_playlists_id_invite: {
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
            /** @description Ссылка-приглашение (плейлист становится совместным) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InviteLink"];
                };
            };
            /** @description Не свой плейлист */
            403: {
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
    reset_invite_post_playlists_id_invite: {
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
            /** @description Новая ссылка; прежняя больше не работает */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["InviteLink"];
                };
            };
            /** @description Не свой плейлист */
            403: {
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
    remove_items: {
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
                "application/json": components["schemas"]["RemovePlaylistItems"];
            };
        };
        responses: {
            /** @description Позиции убраны; чужих и уже убранных среди них не было или они пропущены */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Пустой список или больше 500 позиций */
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
            /** @description Плейлист собирается сам (микс) */
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
    add_members: {
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
                "application/json": components["schemas"]["AddPlaylistMembers"];
            };
        };
        responses: {
            /** @description Друзья — участники (плейлист становится совместным) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Среди людей есть не друзья */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Не свой плейлист */
            403: {
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
    remove_member: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста */
                id: string;
                /** @description Участник; свой id — выйти из плейлиста */
                userId: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Участник убран (или вышел): плейлиста у него больше нет */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Убрать другого может только владелец */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого плейлиста или участника */
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
    move_playlist: {
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
                "application/json": components["schemas"]["MovePlaylist"];
            };
        };
        responses: {
            /** @description Плейлист в папке (или вне папок) на новом месте */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistList"];
                };
            };
            /** @description Соседа нет на этом уровне */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого плейлиста или папки */
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
    set_order: {
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
                "application/json": components["schemas"]["PlaylistOrder"];
            };
        };
        responses: {
            /** @description Порядок задан: названные позиции — так, остальные — за ними в прежнем порядке; чужие id пропущены */
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
            /** @description Плейлист собирается сам (микс) */
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
    apply: {
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
                "application/json": components["schemas"]["SetOptions"];
            };
        };
        responses: {
            /** @description Сет применён; прежний порядок — для «Отменить» */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SetApplied"];
                };
            };
            /** @description Нет позиции `firstItemId` */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Совместный плейлист: собирает только владелец */
            403: {
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
            /** @description Плейлист собирается сам (микс): сначала сохранить копию */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Слишком длинный плейлист (`too_long`) */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    preview_post_playlists_id_set_preview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id плейлиста или микса */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SetOptions"];
            };
        };
        responses: {
            /** @description Порядок сета; плейлист не меняется */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SetPreview"];
                };
            };
            /** @description Нет позиции `firstItemId` */
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
            /** @description Слишком длинный плейлист (`too_long`) */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    set_visibility: {
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
                "application/json": components["schemas"]["PlaylistVisibility"];
            };
        };
        responses: {
            /** @description Видят ли друзья плейлист на странице владельца */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Не свой плейлист */
            403: {
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
    recognize_history: {
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
            /** @description История распознанного, новые первыми */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecognitionPage"];
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
    recognize_delete: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id записи истории */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Запись убрана из истории */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такой записи у этого человека */
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
    recognize_resolve: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RecognizeRequest"];
            };
        };
        responses: {
            /** @description Распознанное записано в историю; `track` — трек каталога (по ISRC, затем поиском) или `null`, если не нашёлся. Аудио скачивается при первом воспроизведении */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Recognition"];
                };
            };
            /** @description Пустые или слишком длинные название и артист, artworkUrl не https */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Deezer не ответил: в историю не записано, стоит повторить */
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
    register: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReleaseRequest"];
            };
        };
        responses: {
            /** @description Выпуск зарегистрирован (та же версия — заменён): приложения получат его при следующей проверке */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["AppRelease"];
                };
            };
            /** @description Неверные поля */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Токен выпусков не принят */
            401: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_uploaded` — файла ещё нет в хранилище */
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
    upload: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ReleaseUploadRequest"];
            };
        };
        responses: {
            /** @description Ссылка на загрузку файла обновления прямо в хранилище (`PUT`, час) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ReleaseUpload"];
                };
            };
            /** @description Неизвестная платформа, не SemVer или плохое имя файла */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Токен выпусков не принят */
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
    resolve_link: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ResolveLinkRequest"];
            };
        };
        responses: {
            /** @description Что нашлось в каталоге по ссылке */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ResolvedLink"];
                };
            };
            /** @description Ссылка понятна, но в каталоге такого нет */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `unsupported_link`: не ссылка на музыку из известных сервисов */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Сервис по ссылке или Deezer не ответил */
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
    years: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Годы с прослушиваниями (новые первыми), открытый год, стиль историй и посмотрены ли они */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RewindYears"];
                };
            };
            /** @description `not_open` — итоги сейчас закрыты (администратору видно всегда) */
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
    year: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Год по времени слушателя */
                year: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Итоги года; текущий — «по сей день» (`complete: false`). Без прослушиваний — нули и пустые списки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Rewind"];
                };
            };
            /** @description Год ещё не наступил; `not_open` — итоги закрыты */
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
    more: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Год по времени слушателя */
                year: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description «Ещё послушать»: новое, похожее на любимое за год */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RewindMore"];
                };
            };
            /** @description Год ещё не наступил; `not_open` — итоги закрыты */
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
    save_more: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Год по времени слушателя */
                year: number;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SaveRewindMore"];
            };
        };
        responses: {
            /** @description Уже был — заменён показанной подборкой */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Плейлист «Ещё послушать {год}» создан */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Нет треков или больше 100 */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Год ещё не наступил; `not_open` — итоги закрыты */
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
    playlist: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Год по времени слушателя */
                year: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Уже был — обновлён по свежим итогам */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description Плейлист «Твои лучшие треки {год}» создан */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["PlaylistSummary"];
                };
            };
            /** @description За этот год ещё нечего собрать */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Год ещё не наступил; `not_open` — итоги закрыты */
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
    seen: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Год по времени слушателя */
                year: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Отмечено: истории открывали — карточка «Твои итоги готовы» больше не зовёт */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Год ещё не наступил; `not_open` — итоги закрыты */
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
    soundtrack: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Год по времени слушателя */
                year: number;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Музыка историй: трек и место начала у слайдов. Треки не на сервере ставятся на скачивание вне очереди */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RewindSoundtrack"];
                };
            };
            /** @description Год ещё не наступил; `not_open` — итоги закрыты */
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
    get_settings_get_settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Настройки слушателя (общие для всех устройств) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserSettings"];
                };
            };
        };
    };
    update_patch_settings: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["UserSettingsPatch"];
            };
        };
        responses: {
            /** @description Настройки после правки; другие устройства получат `library.changed` */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UserSettings"];
                };
            };
            /** @description Неизвестное поле или значение */
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
    card: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description track, album, artist или playlist */
                kind: components["schemas"]["ShareKind"];
                /** @description id сущности */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Ссылка, название, подпись и публичная обложка */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ShareCard"];
                };
            };
            /** @description Нет такого (или плейлист чужой) */
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
    overview_get_taste: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Грани вкуса с примерами, «меньше такого», прослушивания за месяц и из чего волна знает вкус */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TasteOverview"];
                };
            };
        };
    };
    remove_damper: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id приглушения */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description «Меньше такого» отменено (повтор безопасен) */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    less: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id грани */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Грань приглушена: волна и миксы реже берут похожее, её доля во вкусе меньше. Повтор — сильнее (до полной силы) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TasteDamper"];
                };
            };
            /** @description Нет такой грани у слушателя */
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
    reset: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TasteReset"];
            };
        };
        responses: {
            /** @description Вкус сброшен: грани, «меньше такого», миксы и заготовки волны собираются заново по новым прослушиваниям. Медиатека и «не рекомендовать» остаются */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет подтверждения (`confirm: true`) */
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
    train_claim: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TrainClaimBody"];
            };
        };
        responses: {
            /** @description Задание: обучающая часть набора и описание обучения. Отложенной части в нём нет — её проверяет сервер */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrainClaim"];
                };
            };
            /** @description Учить сейчас нечего: сеть недавно училась, мало данных или задание уже у другого компьютера */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Не приложение */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Не компьютер, или общую сеть просит не администратор */
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
    train_fail: {
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
                "application/json": components["schemas"]["TrainFailure"];
            };
        };
        responses: {
            /** @description Задание закрыто; следующее компьютер получит позже */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого задания у этого компьютера */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Задание уже закрыто */
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
    train_result: {
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
                "application/json": components["schemas"]["TrainSubmit"];
            };
        };
        responses: {
            /** @description Веса проверены на отложенной части сервера; включены, только если сеть точнее правил и текущей версии */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrainOutcome"];
                };
            };
            /** @description Общую сеть присылает только администратор */
            403: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого задания у этого компьютера */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Задание закрыто или просрочено */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Веса не приняты: размеры, числа или ONNX не загружается */
            422: {
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
    suggest: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["SuggestRequest"];
            };
        };
        responses: {
            /** @description «Умное перемешивание»: похожее по звучанию на весь список, не из него и не из `exclude`, не больше двух у артиста; подпись — «Подобрано» */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WaveItems"];
                };
            };
            /** @description Пустой список, больше 500 треков или limit вне 1–50 */
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
    track_analysis: {
        parameters: {
            query?: never;
            header?: {
                /** @description ETag прошлого ответа: не изменился — 304 */
                "If-None-Match"?: string | null;
            };
            path: {
                /** @description id трека */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Анализ полного файла записи: доли, фразы по 8 и 16 тактов, точки сведения. Кешируется на час (`private`), ETag меняется с анализом */
            200: {
                headers: {
                    "Cache-Control"?: string;
                    /** @description Версия анализа */
                    ETag?: string;
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackAnalysis"];
                };
            };
            /** @description Анализ не изменился с переданного `If-None-Match` */
            304: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого трека (`not_found`) или полного анализа его записи ещё нет (`analysis_missing`: файл не скачан или не разобран) */
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
    track_credits: {
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
            /** @description Авторы музыки и слов, продюсеры и звукорежиссёры (MusicBrainz по ISRC), лейбл и год. Пустые списки — MusicBrainz трека не знает или не ответил. `resolving` — ссылки на страницы людей ещё ищутся */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackCredits"];
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
    hook: {
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
            /** @description Отрывок ~15 с с припева: из звука на сервере (место — по анализу или 30 % трека) или превью Deezer. Трек не ставится на скачивание */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackHook"];
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
    track_lyrics: {
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
            /** @description Текст трека: строки с временем (`synced`), простой текст (`plain`), «без слов» (`instrumental`) или «не нашли» (`none`). Берётся у LRCLIB при первом запросе и хранится; «не нашли» — 30 дней */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackLyrics"];
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
            /** @description LRCLIB не ответил или просит подождать: стоит повторить позже */
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
            query?: {
                /** @description Явное качество; без него — по `network` и настройкам слушателя. */
                quality?: components["schemas"]["StreamQuality"];
                /** @description Сеть устройства; без неё — Wi-Fi (сайт). */
                network?: components["schemas"]["NetworkKind"];
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
            /** @description `status: ready` — подписанная ссылка на файл (6 часов), формат, громкость, форма волны. Файл — под качество: явное `quality` или по `network` и настройкам слушателя; «Экономия» до готовности копии отдаёт обычный файл (`quality: normal`) */
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
    track_versions: {
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
            /** @description Версии той же песни: оригинал, slowed, super slowed, sped up; раз в две недели ищутся у Deezer заново */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TrackVersions"];
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
    choose: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека: выбор запоминается для его основного исполнителя */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["VersionChoiceBody"];
            };
        };
        responses: {
            /** @description Запомнено; `original` — выбор забыт */
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
    report_wrong_recording: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id трека */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["WrongRecordingReport"];
            };
        };
        responses: {
            /** @description Жалоба принята: спорная запись в «Спорных» админки и новое скачивание вне очереди без видео этой и прежних жалоб; новый файл заменит основной */
            202: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WrongRecordingResult"];
                };
            };
            /** @description Комментарий длиннее 500 символов */
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
            /** @description У трека ещё нет файла на сервере (`not_ready`) */
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
    check: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description Платформа: `darwin`, `windows`, `linux` */
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
            /** @description Есть версия новее: формат tauri-plugin-updater */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["UpdateManifest"];
                };
            };
            /** @description Обновлений нет */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Версия — не SemVer */
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
    choices: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Исполнители, у которых слушатель выбрал не оригинал */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VersionChoices"];
                };
            };
        };
    };
    list_get_wave_presets: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сохранённые сочетания по порядку карточек */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WavePresetList"];
                };
            };
        };
    };
    create_post_wave_presets: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewWavePreset"];
            };
        };
        responses: {
            /** @description Сочетание сохранено в конец ряда */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WavePreset"];
                };
            };
            /** @description Пустое или длинное название, неверная иконка, радио в настройках, сочетаний уже 24 */
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
    reorder: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["WavePresetOrder"];
            };
        };
        responses: {
            /** @description Сочетания в новом порядке */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WavePresetList"];
                };
            };
        };
    };
    remove_delete_wave_presets_id_: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id сочетания */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Сочетание удалено */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            /** @description Нет такого сочетания */
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
    update_patch_wave_presets_id_: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description id сочетания */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["WavePresetPatch"];
            };
        };
        responses: {
            /** @description Сочетание после правки */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WavePreset"];
                };
            };
            /** @description Пустое или длинное название, неверная иконка, радио в настройках */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description Нет такого сочетания */
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
            /** @description Сессия волны; с `seed` — радио от трека, артиста, альбома, плейлиста или описания. Начало (`queue`) уже подобрано под настройки. `state = preparing` — готовых на сервере меньше `need`: лучшие из неготовых качаются вне очереди, ход — `GET` сессии (и `acquisition.updated` по трекам очереди); не дольше полутора минут */
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
            /** @description Сессия: настройки, подпись, сколько выдано и ход подготовки начала: готовое — первым; что не скачалось — заменено; за полторы минуты не успели — начало из того, что уже на сервере */
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
    why: {
        parameters: {
            query: {
                trackId: string;
                /** @description Сессия волны, из которой трек; без неё — последний показ трека. */
                sessionId?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description «Почему этот трек»: что на самом деле повлияло на выбор, понятными словами. Администратору — ещё и оценки, сходства и версия сети (`details`) */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["WhyTrack"];
                };
            };
            /** @description Волна не предлагала этот трек слушателю */
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
