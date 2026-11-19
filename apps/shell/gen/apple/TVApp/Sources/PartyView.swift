import SwiftUI

/// Вечеринка: гости по QR добавляют треки без аккаунта («Слушать вместе»), настройки волны на вечер.
/// Включённая вечеринка показывает QR и на сцене «Сейчас играет».
struct PartyView: View {
    @EnvironmentObject private var store: Store

    private var active: Bool { store.party?.active == true }

    var body: some View {
        HStack(alignment: .top, spacing: 110) {
            VStack(alignment: .leading, spacing: 30) {
                Text("Вечеринка").font(.system(size: 64, weight: .bold)).foregroundStyle(Theme.text1)
                Text(active
                     ? String(localized: "Гости наводят камеру на код и добавляют треки — без аккаунта.")
                     : String(localized: "Сцена на весь экран и QR для гостей: они добавляют треки со своих телефонов."))
                    .font(.system(size: 30))
                    .foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)

                Button {
                    store.updateParty(["active": !active])
                    if !active { store.tab = .now }
                } label: {
                    Label(active ? String(localized: "Закончить вечеринку") : String(localized: "Начать вечеринку"),
                          systemImage: active ? "stop.fill" : "party.popper.fill")
                        .font(.system(size: 30, weight: .semibold))
                        .padding(.horizontal, 20)
                }
                .padding(.top, 10)

                VStack(alignment: .leading, spacing: 18) {
                    toggle(String(localized: "Только энергичное"), on: store.party?.settings.energetic ?? false) {
                        store.updateParty(["energetic": $0])
                    }
                    toggle(String(localized: "Без повторов артистов"), on: store.party?.settings.noArtistRepeat ?? true) {
                        store.updateParty(["noArtistRepeat": $0])
                    }
                    toggle(String(localized: "Диджей"), on: store.party?.settings.dj ?? true) {
                        store.updateParty(["dj": $0])
                    }
                }
                .padding(.top, 20)
                .frame(maxWidth: 820)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            VStack(alignment: .leading, spacing: 30) {
                if active, let url = store.jam?.url {
                    QRCode(value: url, size: 440)
                    Text("Добавить трек").font(.system(size: 30, weight: .semibold)).foregroundStyle(Theme.text1)
                } else {
                    RoundedRectangle(cornerRadius: 36, style: .continuous)
                        .fill(Theme.fill1)
                        .frame(width: 440, height: 440)
                        .overlay(Image(systemName: "qrcode").font(.system(size: 120)).foregroundStyle(Theme.text3))
                }
                if let guests = store.jam?.guests, !guests.isEmpty {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Гости").font(.system(size: 26, weight: .semibold)).foregroundStyle(Theme.text3)
                        Text(guests.map(\.name).joined(separator: ", "))
                            .font(.system(size: 28))
                            .foregroundStyle(Theme.text2)
                            .lineLimit(3)
                    }
                    .frame(width: 440, alignment: .leading)
                }
            }
        }
        .padding(.horizontal, 110)
        .padding(.vertical, 60)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.ink.ignoresSafeArea())
        .task { await store.loadParty() }
    }

    /// Переключатель tvOS: строка-кнопка, состояние справа.
    private func toggle(_ title: String, on: Bool, change: @escaping (Bool) -> Void) -> some View {
        Button { change(!on) } label: {
            HStack {
                Text(title).font(.system(size: 30, weight: .medium))
                Spacer()
                Text(on ? String(localized: "Вкл.") : String(localized: "Выкл.")).font(.system(size: 28)).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 10)
        }
    }
}
