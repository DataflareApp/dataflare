import { DEMO_CONNECTION_ID } from '../../tauri'
import { TabType, useTabsStore } from './hooks/use-store'

export const openDefaultDemoTabs = (cid: string) => {
    if (cid !== DEMO_CONNECTION_ID) {
        return
    }

    const TABS_OPENED_KEY = 'DemoDatabaseDefaultTabsOpened'
    if (localStorage.getItem(TABS_OPENED_KEY) === '1') {
        return
    }

    const { switchTabTo } = useTabsStore.getState()
    switchTabTo({ type: TabType.SchemaManager })
    switchTabTo({ type: TabType.Dashboard })

    localStorage.setItem(TABS_OPENED_KEY, '1')
}
