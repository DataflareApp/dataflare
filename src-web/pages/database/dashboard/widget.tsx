import { IconChartAreaLine, IconChartPie, IconDots, IconProgress, IconTrendingUp } from '@tabler/icons-react'
import { NodeResizeControl, NodeProps } from '@xyflow/react'
import { memo } from 'react'
import { useTranslation } from '../../../i18n'
import { MetricConfig, ProgressConfig, ProgressShape, Query, WidgetConfig, WidgetType } from '../../../tauri'
import {
    IconButton,
    IconRefresh,
    ComposedChart,
    PieChart,
    DropdownMenu,
    DropdownMenuItem,
    DropdownMenuSeparator,
    showMessageBox,
    showRenameDialog,
    ErrorMessage,
    Loading,
    Message
} from '../../../ui'
import { TableType } from '../db/db-types'
import { TableIcon } from '../icon'
import { Table } from '../table'
import { displayDatabaseValue } from '../table/utils'
import { useWidgetNodes, useWidgetQuery } from './hooks'

const MIN_SIZE = 192

export type EditorWidgetData = {
    wid: string | null
    config: WidgetConfig
}

export const Widget = ({
    id: wid,
    data: config,
    width = MIN_SIZE,
    height = MIN_SIZE,
    positionAbsoluteX,
    positionAbsoluteY,
    onEditWidget
}: Omit<NodeProps, 'data'> & {
    data: WidgetConfig
    onEditWidget: (data: EditorWidgetData) => void
}) => {
    const { t, tf } = useTranslation()
    const { data, mutate, isValidating, isLoading, error } = useWidgetQuery(
        wid,
        config.source,
        config.interval
    )
    const { createWidget, deleteWidget, updateWidgetConfig } = useWidgetNodes()

    const onRename = () => {
        showRenameDialog({
            from: config.name,
            onHandler: (name) =>
                updateWidgetConfig(wid, {
                    ...config,
                    name
                }),
            onSuccess: () => {}
        })
    }
    const onEdit = () => {
        onEditWidget({
            wid,
            config: structuredClone(config)
        })
    }
    const onDuplicate = () => {
        createWidget(
            structuredClone(config),
            Math.round(positionAbsoluteX) + 36,
            Math.round(positionAbsoluteY) + 36,
            width,
            height
        )
    }
    const onDelete = () => {
        showMessageBox(t('deleteWidegt'), tf('deleteMessage', config.name), 'delete', {
            label: t('delete'),
            primary: true,
            onClick: () => deleteWidget(wid)
        })
    }

    return (
        <div className='group h-full rounded border border-separator bg-main shadow-lg'>
            <header className='flex h-9 items-center gap-2 border-b border-separator px-4'>
                <WidgetIcon type={config.options.type} />
                <span className='flex-1 truncate text-sm text-primary'>{config.name}</span>
                <IconButton
                    title={t('refresh')}
                    disabled={isValidating}
                    className={
                        'nodrag focus:opacity-100 group-hover:opacity-100 ' +
                        (isValidating ? '' : 'opacity-0')
                    }
                    onClick={() => mutate()}
                >
                    <IconRefresh loading={isValidating} />
                </IconButton>
                <DropdownMenu
                    trigger={
                        <IconButton title={t('option')} className='-ml-2'>
                            <IconDots size={16} strokeWidth={1.5} className='fill-current' />
                        </IconButton>
                    }
                >
                    <DropdownMenuItem onClick={onRename}>{t('rename')}</DropdownMenuItem>
                    <DropdownMenuItem onClick={onEdit}>{t('edit')}</DropdownMenuItem>
                    <DropdownMenuItem onClick={onDuplicate}>{t('duplicate')}</DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem onClick={onDelete}>{t('delete')}</DropdownMenuItem>
                </DropdownMenu>
            </header>
            <div style={{ height: height - 38 }}>
                {isLoading ? (
                    <Loading />
                ) : error !== undefined ? (
                    <ErrorMessage text={error.toString()} />
                ) : (
                    data !== undefined && <WidgetContent wid={wid} config={config} query={data} />
                )}
            </div>
            <NodeResizeControl
                minHeight={MIN_SIZE}
                minWidth={MIN_SIZE}
                className='hidden animate-overlayIn !border-0 !bg-transparent group-hover:block'
            >
                <svg
                    className='-ml-2 -mt-2 h-5 w-5 fill-none text-secondary hover:text-primary'
                    viewBox='0 0 24 24'
                    strokeWidth={1.4}
                    stroke='currentColor'
                    strokeLinecap='round'
                    strokeLinejoin='round'
                >
                    <path d='M20 6V20H6 M16 8V16H8' />
                </svg>
            </NodeResizeControl>
        </div>
    )
}

export const WidgetContent = memo(
    ({ wid, config, query }: { wid: string; config: WidgetConfig; query: Query }): React.JSX.Element => {
        switch (config.options.type) {
            case WidgetType.ComposedChart: {
                return <ComposedChart query={query} config={config.options.config} />
            }
            case WidgetType.Table: {
                return (
                    <div className='nowheel flex size-full cursor-default'>
                        <Table readonly error={undefined} saveColumnSizeID={wid} data={query} />
                    </div>
                )
            }
            case WidgetType.PieChart: {
                return <PieChart query={query} config={config.options.config} />
            }
            case WidgetType.Metric: {
                return <Metric query={query} config={config.options.config} />
            }
            case WidgetType.Progress: {
                return <Progress query={query} config={config.options.config} />
            }
        }
    }
)

const Metric = ({ query, config }: { query: Query; config: MetricConfig }): React.JSX.Element => {
    const { t, numberUtil } = useTranslation()

    if (query.rows.length === 0 || query.columns.length === 0) {
        return <Message text={t('noRows')} />
    }

    const value = query.rows[0][0]

    let text: string
    switch (typeof value) {
        case 'string': {
            text = value
            break
        }
        case 'number':
        case 'bigint': {
            text = numberUtil.format(value)
            break
        }
        default: {
            const MAX = 256
            text = displayDatabaseValue(value)
            if (text.length > MAX) {
                text = text.slice(0, MAX) + '…'
            }
            break
        }
    }

    const start = config.prefix.trimStart()
    const end = config.suffix.trimEnd()
    const displayText = `${start}${text}${end}`

    return (
        <div className='flex size-full cursor-default items-center justify-center p-4'>
            <span
                className='block max-w-full overflow-hidden text-ellipsis whitespace-nowrap font-jb tabular-nums'
                style={{ color: config.color, fontSize: config.fontSize }}
            >
                {displayText}
            </span>
        </div>
    )
}

const Progress = ({ query, config }: { query: Query; config: ProgressConfig }): React.JSX.Element => {
    const { t, numberUtil } = useTranslation()

    if (query.rows.length === 0 || query.columns.length === 0) {
        return <Message text={t('noRows')} />
    }

    const minSafeBigInt = BigInt(Number.MIN_SAFE_INTEGER)
    const maxSafeBigInt = BigInt(Number.MAX_SAFE_INTEGER)
    const compactNumberUtil = new Intl.NumberFormat(numberUtil.resolvedOptions().locale, {
        notation: 'compact',
        maximumFractionDigits: 2
    })

    const readNumber = (dataKey: string): { numeric: number; text: string; title: string } | null => {
        const index = query.columns.findIndex((column) => column.name === dataKey)
        if (index < 0) {
            return null
        }

        const rawValue = query.rows[0][index]
        let numeric: number

        switch (typeof rawValue) {
            case 'number': {
                if (!Number.isFinite(rawValue)) {
                    return null
                }
                numeric = rawValue
                break
            }
            case 'bigint': {
                if (rawValue < minSafeBigInt || rawValue > maxSafeBigInt) {
                    return null
                }
                numeric = Number(rawValue)
                break
            }
            case 'string': {
                if (rawValue.trim() === '') {
                    return null
                }
                numeric = Number(rawValue)
                if (!Number.isFinite(numeric)) {
                    return null
                }
                break
            }
            default: {
                return null
            }
        }

        return {
            numeric,
            text: Math.abs(numeric) >= 1000 ? compactNumberUtil.format(numeric) : numberUtil.format(numeric),
            title: numberUtil.format(numeric)
        }
    }

    const valueData = readNumber(config.valueDataKey)
    const goalData = readNumber(config.goalDataKey)
    if (valueData === null || goalData === null || goalData.numeric === 0) {
        return <Message text='N/A' />
    }

    const percentage = (valueData.numeric / goalData.numeric) * 100
    if (!Number.isFinite(percentage)) {
        return <Message text='N/A' />
    }

    const displayPercentage = Math.trunc(percentage * 100) / 100
    const percentageText = `${displayPercentage}%`
    const renderedPercentage = Math.min(Math.max(percentage, 0), 100)
    const radius = 50 - config.thickness / 2

    switch (config.shape) {
        case ProgressShape.Circular: {
            return (
                <div className='flex size-full min-w-0 cursor-default flex-col overflow-hidden px-4 py-2'>
                    <div
                        role='progressbar'
                        aria-valuemin={0}
                        aria-valuemax={100}
                        aria-valuenow={renderedPercentage}
                        aria-valuetext={percentageText}
                        className='relative flex min-h-0 flex-1 items-center justify-center'
                    >
                        <svg className='absolute inset-0 size-full -rotate-90' viewBox='0 0 100 100'>
                            <g fill='none' strokeWidth={config.thickness}>
                                <circle
                                    cx='50'
                                    cy='50'
                                    r={radius}
                                    stroke='currentColor'
                                    className='text-neutral-200 dark:text-neutral-800'
                                />
                                <circle
                                    cx='50'
                                    cy='50'
                                    r={radius}
                                    pathLength='100'
                                    stroke={config.color}
                                    strokeLinecap='round'
                                    strokeDasharray='100'
                                    strokeDashoffset={100 - renderedPercentage}
                                    className='transition-all duration-500 ease-out motion-reduce:transition-none'
                                />
                            </g>
                        </svg>
                        <span className='relative truncate font-jb text-2xl tabular-nums text-primary'>
                            {percentageText}
                        </span>
                    </div>
                    <div className='flex min-w-0 shrink-0 justify-center gap-3 overflow-hidden pt-2'>
                        <div title={valueData.title} className='flex h-3 min-w-0 items-center gap-1 text-xs'>
                            <span
                                className='h-full w-1 shrink-0 rounded-sm'
                                style={{ backgroundColor: config.color }}
                            />
                            <span className='shrink-0 text-tertiary'>{t('value')}</span>
                            <span className='min-w-0 truncate font-jb tabular-nums text-primary'>
                                {valueData.text}
                            </span>
                        </div>
                        <div title={goalData.title} className='flex h-3 min-w-0 items-center gap-1 text-xs'>
                            <span className='h-full w-1 shrink-0 rounded-sm bg-neutral-200 dark:bg-neutral-800' />
                            <span className='shrink-0 text-tertiary'>{t('goal')}</span>
                            <span className='min-w-0 truncate font-jb tabular-nums text-primary'>
                                {goalData.text}
                            </span>
                        </div>
                    </div>
                </div>
            )
        }
    }
}

const WidgetIcon = ({ type }: { type: WidgetType }): React.JSX.Element => {
    switch (type) {
        case WidgetType.ComposedChart: {
            return <IconChartAreaLine size={16} stroke={1.5} className='text-indigo-500' />
        }
        case WidgetType.Table: {
            return <TableIcon type={TableType.Table} />
        }
        case WidgetType.PieChart: {
            return <IconChartPie size={16} stroke={1.5} className='text-teal-500' />
        }
        case WidgetType.Metric: {
            return <IconTrendingUp size={16} stroke={1.5} className='text-green-500' />
        }
        case WidgetType.Progress: {
            return <IconProgress size={16} stroke={1.5} className='text-orange-500' />
        }
    }
}
