import {
    Layout,
    ProgressShape,
    WidgetComposedChartConfig,
    WidgetMetricConfig,
    WidgetPieChartConfig,
    WidgetProgressConfig,
    WidgetTableConfig,
    WidgetType
} from '../../../tauri'
import { CHART_COLORS } from '../../../ui'

export const INTERVALS = [0, 5, 20, 60]
const DEFAULT_INTERVAL = 20

export const DEFAULT_CATEGORY_KEY = ''

export const defaultComposedChartConfig = (): WidgetComposedChartConfig => {
    return {
        name: 'New Chart',
        source: '\n\n',
        interval: DEFAULT_INTERVAL,
        options: {
            type: WidgetType.ComposedChart,
            config: {
                layout: Layout.Horizontal,
                categoryDataKey: '',
                axis: {
                    x: {
                        hidden: false
                    },
                    y: {
                        hidden: true
                    }
                },
                areas: [],
                bars: [],
                lines: []
            }
        }
    }
}

export const defaultTableConfig = (): WidgetTableConfig => {
    return {
        name: 'New Table',
        source: '\n\n',
        interval: DEFAULT_INTERVAL,
        options: {
            type: WidgetType.Table,
            config: {}
        }
    }
}

export const defaultPieChartConfig = (): WidgetPieChartConfig => {
    return {
        name: 'New Pie Chart',
        source: '\n\n',
        interval: DEFAULT_INTERVAL,
        options: {
            type: WidgetType.PieChart,
            config: {
                nameKey: '',
                dataKey: '',
                startColorIndex: randomPieColorIndex()
            }
        }
    }
}

export const defaultMetricConfig = (): WidgetMetricConfig => {
    return {
        name: 'New Metric',
        source: '\n\n',
        interval: DEFAULT_INTERVAL,
        options: {
            type: WidgetType.Metric,
            config: {
                prefix: '',
                suffix: '',
                color: CHART_COLORS[randomPieColorIndex()],
                fontSize: 24
            }
        }
    }
}

export const defaultProgressConfig = (): WidgetProgressConfig => {
    return {
        name: 'New Progress',
        source: '\n\n',
        interval: DEFAULT_INTERVAL,
        options: {
            type: WidgetType.Progress,
            config: {
                shape: ProgressShape.Circular,
                valueDataKey: '',
                goalDataKey: '',
                thickness: 10,
                color: CHART_COLORS[randomPieColorIndex()]
            }
        }
    }
}

export const randomPieColorIndex = (currentIndex?: number): number => {
    if (currentIndex === undefined || CHART_COLORS.length < 2) {
        return Math.floor(Math.random() * CHART_COLORS.length)
    }
    while (true) {
        const i = Math.floor(Math.random() * CHART_COLORS.length)
        if (i !== currentIndex) {
            return i
        }
    }
}
