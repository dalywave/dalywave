import SwiftUI

/// «Моя волна»: запуск одним нажатием, настроения и сохранённые сочетания (как в настройках волны).
struct WaveView: View {
    @EnvironmentObject private var store: Store

    private let columns = [GridItem(.flexible(), spacing: 6), GridItem(.flexible(), spacing: 6)]

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 12) {
                    WaveButton(size: 66) { store.startWave() }

                    LazyVGrid(columns: columns, spacing: 6) {
                        ForEach(Mood.allCases) { mood in
                            Button {
                                store.startWave(WaveSettings(mood: mood.rawValue))
                            } label: {
                                VStack(spacing: 3) {
                                    Image(systemName: mood.symbol)
                                        .font(.system(size: 15, weight: .semibold))
                                        .foregroundStyle(mood.color)
                                    Text(mood.title)
                                        .font(.system(size: 14, weight: .medium))
                                        .foregroundStyle(Theme.text1)
                                        .lineLimit(1)
                                        .minimumScaleFactor(0.75)
                                }
                                .padding(.horizontal, 4)
                                .frame(maxWidth: .infinity)
                                .frame(height: 52)
                                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.fill1))
                            }
                            .buttonStyle(.plain)
                        }
                    }

                    if !store.presets.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Saved")
                                .font(.footnote.weight(.semibold))
                                .foregroundStyle(Theme.text3)
                                .padding(.leading, 4)
                            ForEach(store.presets) { preset in
                                Button {
                                    store.startWave(preset.settings)
                                } label: {
                                    HStack(spacing: 10) {
                                        Image(systemName: presetSymbol(preset.icon))
                                            .font(.system(size: 14, weight: .semibold))
                                            .foregroundStyle(Theme.text2)
                                            .frame(width: 20)
                                        Text(preset.name)
                                            .font(.body)
                                            .foregroundStyle(Theme.text1)
                                            .lineLimit(1)
                                        Spacer(minLength: 0)
                                    }
                                    .padding(.horizontal, 10)
                                    .frame(minHeight: 44)
                                    .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.fill1))
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }
                .padding(.horizontal, 2)
                .padding(.bottom, 14)
            }
            .task { await store.loadLibrary() }
        }
    }
}
