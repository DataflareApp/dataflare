import { IconCirclesRelation } from '@tabler/icons-react'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { Fragment } from 'react'
import useSWR from 'swr'
import { t, useTranslation } from '../../../i18n'
import { ConnectionInfoItem, ConnectionInfoValueKind, Database } from '../../../tauri'
import {
    ErrorMessage,
    IconButton,
    IconRefresh,
    Loading,
    Popover,
    popoverSize,
    ScrollView,
    showMessageBox
} from '../../../ui'
import { openURL } from '../../../utils/opener'
import { useConnectID } from '../hooks/use-store'

export const ConnectionInfo = () => {
    const { t } = useTranslation()

    return (
        <Popover
            trigger={
                <IconButton title={t('connectionInfo')}>
                    <IconCirclesRelation size={16} strokeWidth={1.5} />
                </IconButton>
            }
            onOpenAutoFocus={(e) => e.preventDefault()}
        >
            <ConnectionInfoContent />
        </Popover>
    )
}

const ConnectionInfoContent = () => {
    const { t } = useTranslation()
    const connectID = useConnectID()
    const key = [connectID, 'connection-info'] as const
    const { data, error, isLoading, isValidating, mutate } = useSWR(key, Database.info)

    return (
        <>
            <div className='flex items-center justify-between gap-2 px-4'>
                <span className='text-sm leading-10 text-primary'>{t('connectionInfo')}</span>
                <IconButton title={t('refresh')} disabled={isValidating} onClick={() => mutate()}>
                    <IconRefresh loading={isValidating} />
                </IconButton>
            </div>
            {isLoading ? (
                <div className='h-32 w-72'>
                    <Loading />
                </div>
            ) : data === undefined && error !== undefined ? (
                <div className='h-32 min-w-72 max-w-96'>
                    <ErrorMessage text={error} />
                </div>
            ) : data === undefined ? (
                <div className='h-32 w-72'>
                    <Loading />
                </div>
            ) : (
                <ScrollView
                    axis='y'
                    viewportClassName='px-3 pb-3 px-4 min-w-72'
                    style={{
                        maxHeight: `calc(${popoverSize.maxHeight} - 40px)`,
                        maxWidth: `min(${popoverSize.maxWidth}, 520px)`
                    }}
                >
                    <div className='grid select-text grid-cols-[auto,1fr] gap-x-3 gap-y-2 break-all rounded bg-neutral-200/20 px-3 py-2 text-xs dark:bg-neutral-800/20'>
                        {data.items.map((item, index) => {
                            return (
                                <Fragment key={item.name}>
                                    <div className='text-right text-tertiary'>{item.name}</div>
                                    <InfoValue item={item} />
                                </Fragment>
                            )
                        })}
                    </div>
                </ScrollView>
            )}
        </>
    )
}

const InfoValue = ({ item }: { item: ConnectionInfoItem }) => {
    switch (item.value.kind) {
        case ConnectionInfoValueKind.Text: {
            return (
                <div
                    className='min-w-0 whitespace-pre-wrap break-words text-secondary'
                    title={item.value.text}
                    onContextMenu={(e) => e.stopPropagation()}
                >
                    {item.value.text}
                </div>
            )
        }
        case ConnectionInfoValueKind.File: {
            const path = item.value.path
            const onClick = () => {
                revealItemInDir(path).catch((err) => {
                    showMessageBox(t('error'), err, 'error')
                })
            }
            return (
                <div
                    className='min-w-0 break-words text-theme hover:underline'
                    title={path}
                    onClick={onClick}
                    onContextMenu={(e) => e.stopPropagation()}
                >
                    {path}
                </div>
            )
        }
        case ConnectionInfoValueKind.Url: {
            const { url } = item.value
            return (
                <div
                    className='min-w-0 text-theme hover:underline'
                    title={url}
                    onClick={() => openURL(url)}
                    onContextMenu={(e) => e.stopPropagation()}
                >
                    {url}
                </div>
            )
        }
        case ConnectionInfoValueKind.Server: {
            return (
                <div className='min-w-0 break-all text-secondary' onContextMenu={(e) => e.stopPropagation()}>
                    <span className='mr-1 rounded border border-theme px-1 text-theme'>
                        {item.value.protocol}
                    </span>
                    <span className='font-jb'>{item.value.server}</span>
                </div>
            )
        }
    }
}
