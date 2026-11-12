#include "bindings/bindings.h"

// Apple Watch (WatchLink.swift): WCSession — до запуска приложения, часы могут разбудить его в фоне
extern "C" void dalywave_watch_start(void);
// Push (PushLink.swift): делегат уведомлений — до запуска, нажатие могло и запустить приложение
extern "C" void dalywave_push_start(void);

int main(int argc, char * argv[]) {
	dalywave_watch_start();
	dalywave_push_start();
	ffi::start_app();
	return 0;
}
