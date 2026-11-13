import Foundation
import TVServices

/// Снимок для верхней полки: недавнее и плейлисты (обложки — полными адресами сервера).
enum TopShelf {
    static func save(recent: [RecentItem], playlists: [PlaylistSummary], api: Api) {
        let url = { (path: String?) in path.flatMap { api.imageURL($0)?.absoluteString } }
        TopShelfSnapshot(
            recent: recent.prefix(12).map {
                .init(id: "\($0.type)-\($0.id)", title: $0.title, imageURL: url($0.cover),
                      link: TopShelfSnapshot.link(type: $0.type, id: $0.id, title: $0.title))
            },
            playlists: playlists.prefix(12).map {
                .init(id: "playlist-\($0.id)", title: $0.title, imageURL: url($0.cover),
                      link: TopShelfSnapshot.link(type: "playlist", id: $0.id, title: $0.title))
            }
        ).save()
        TVTopShelfContentProvider.topShelfContentDidChange()
    }
}
