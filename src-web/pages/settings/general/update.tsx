import { IconRefreshAlert } from '@tabler/icons-react'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, Update } from '@tauri-apps/plugin-updater'
import { useEffect, useState } from 'react'
import useSWRImmutable from 'swr/immutable'
import useSWRMutation from 'swr/mutation'
import { useTranslation } from '../../../i18n'
import { getWaitAppRestart, isPortable, setAppUpdateAvailable, setWaitAppRestart } from '../../../tauri'
import { Button, IconButton, IconRefresh, showMessageBox } from '../../../ui'
import { openURL, PORTABLE_UPDATE_URL } from '../../../utils/opener'

export const UpdateSettings = () => {
    const { t } = useTranslation()
    const [waitRestart, setWaitRestart] = useState<null | boolean>(null)

    useEffect(() => {
        getWaitAppRestart().then(setWaitRestart)
    }, [])

    if (waitRestart === null) {
        return <div className='h-7' />
    }

    if (waitRestart) {
        return (
            <Button primary onClick={relaunch} className='w-48'>
                {t('restart')}
            </Button>
        )
    }

    return (
        <CheckUpdate
            onInstallSuccess={() => {
                setWaitRestart(true)
                setWaitAppRestart()
            }}
        />
    )
}

const CheckUpdate = ({ onInstallSuccess }: { onInstallSuccess: () => void }) => {
    const { data, isLoading, error, mutate } = useSWRImmutable('check-update', async () => {
        const update = await check()
        setAppUpdateAvailable(update !== null)
        return update
    })

    if (error) {
        return <Failed message={error} onRetry={() => mutate(undefined)} />
    }
    if (isLoading) {
        return <Checking />
    }
    if (data === null || data === undefined) {
        return <NoUpdate />
    }
    return (
        <InstallUpdate update={data} onRetry={() => mutate(undefined)} onInstallSuccess={onInstallSuccess} />
    )
}

const InstallUpdate = ({
    update,
    onRetry,
    onInstallSuccess
}: {
    update: Update
    onRetry: () => void
    onInstallSuccess: () => void
}) => {
    const { t, tf } = useTranslation()
    const { error, isMutating, trigger } = useSWRMutation('install-update', async () => {
        // TODO: Implement portable update self
        // https://github.com/mitsuhiko/self-replace
        if (await isPortable()) {
            openURL(PORTABLE_UPDATE_URL)
            return false
        }
        // NOTE: On Windows, this restarts immediately after installation
        await update.downloadAndInstall()
        return true
    })

    const onInstallUpdate = async () => {
        const installed = await trigger()
        if (!installed) {
            return
        }
        onInstallSuccess()
        // NOTE: Windows will not reach this point
        showMessageBox(tf('newVersionInstalled', update.version), t('restartMessage'), 'success', [
            {
                label: t('restart'),
                primary: true,
                onClick() {
                    relaunch()
                }
            }
        ])
    }

    if (error) {
        return <Failed message={error} onRetry={onRetry} />
    }

    if (isMutating) {
        return <Button primary loading={true} className='w-48' />
    }

    return (
        <button
            className='flex h-7 w-48 items-center justify-center gap-3 rounded border border-yellow-500 text-yellow-600 transition-colors hover:border-yellow-600 hover:bg-yellow-600 hover:text-white'
            onClick={onInstallUpdate}
        >
            <IconRefreshAlert size={16} stroke={1.5} />
            {t('installNow')}
        </button>
    )
}

const Checking = () => {
    const { t } = useTranslation()
    return (
        <div className='flex h-7 items-center gap-2 text-tertiary'>
            <span>{t('checkingForUpdates')}</span>
            <IconRefresh loading />
        </div>
    )
}

const NoUpdate = () => {
    const { t } = useTranslation()
    return <div className='leading-7'>{t('latestVersion')}</div>
}

const Failed = ({ message, onRetry }: { message: string; onRetry: () => void }) => {
    return (
        <div className='flex min-h-7 items-center'>
            <span
                className='grow select-text break-all text-red-500'
                onContextMenu={(e) => e.stopPropagation()}
            >
                {message}
            </span>
            <IconButton onClick={onRetry}>
                <IconRefresh loading={false} />
            </IconButton>
        </div>
    )
}
