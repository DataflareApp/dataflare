import { IconEye, IconEyeClosed, IconHelp } from '@tabler/icons-react'
import clsx from 'clsx'
import { Fragment, useState } from 'react'
import { useTranslation } from '../i18n'
import { IconButton } from './button'
import { HoverCard } from './hover-card'
import { TextInput } from './input'

export interface PasswordInputProps {
    className?: string
    placeholder?: string
    value: string
    onChange: (value: string) => void
}

type SecretResolveType = 'ENV' | 'FILE' | 'EXEC'

// NOTE: Keep these prefixes in sync with src-crates/secret-resolve/secret_resolve.rs.
const getSecretResolveType = (value: string): SecretResolveType | null => {
    const trimmed = value.trimStart()
    if (trimmed.startsWith('env:')) {
        return 'ENV'
    }
    if (trimmed.startsWith('file:')) {
        return 'FILE'
    }
    if (trimmed.startsWith('exec:')) {
        return 'EXEC'
    }
    return null
}

export const PasswordInput = ({ className, value, placeholder, onChange }: PasswordInputProps) => {
    const { t } = useTranslation()
    const [show, setShow] = useState(false)
    const secretResolveType = getSecretResolveType(value)

    return (
        <div className={clsx('flex items-center gap-2', className)}>
            <div className='relative h-7 min-w-0 flex-1'>
                <input
                    type={secretResolveType !== null || show ? 'text' : 'password'}
                    className={clsx(
                        'block size-full rounded border border-separator bg-transparent pl-2 text-sm text-secondary placeholder-quarternary',
                        secretResolveType ? 'pr-12' : 'pr-10'
                    )}
                    placeholder={placeholder}
                    onContextMenu={(e) => e.stopPropagation()}
                    spellCheck='false'
                    autoComplete='off'
                    autoCapitalize='none'
                    value={value}
                    onChange={(e) => onChange(e.target.value)}
                />
                {secretResolveType !== null && (
                    <span className='pointer-events-none absolute right-1 top-1/2 flex h-5 -translate-y-1/2 items-center rounded border border-theme/30 bg-theme/10 px-1.5 font-jb text-[10px] font-medium text-theme'>
                        {secretResolveType}
                    </span>
                )}
                {secretResolveType === null && (
                    <IconButton
                        title={show ? t('hiddenPassword') : t('showPassword')}
                        className='absolute right-0 top-0 h-full'
                        onClick={() => setShow(!show)}
                    >
                        {show ? <IconEye size={16} stroke={1.7} /> : <IconEyeClosed size={16} stroke={1.7} />}
                    </IconButton>
                )}
            </div>
            <PasswordSecretResolve />
        </div>
    )
}

const PasswordSecretResolve = () => {
    const { t } = useTranslation()
    const items = [
        {
            label: 'From shell output:',
            example: "exec: echo 'MY_PASSWORD'"
        },
        {
            label: 'From env:',
            example: 'env: MY_PASSWORD'
        },
        {
            label: 'From env file:',
            example: 'env: /path/.env#MY_PASSWORD'
        },
        {
            label: 'From file content:',
            example: 'file: /path/file'
        }
    ]
    return (
        <HoverCard
            openDelay={100}
            closeDelay={100}
            trigger={<IconHelp size={16} stroke={1.5} className='text-tertiary hover:text-primary' />}
            side='left'
        >
            <div className='max-w-80 px-4 py-3'>
                <div className='text-xs leading-4'>{t('secretResolveMsg')}</div>
                {items.map((item) => {
                    return (
                        <Fragment key={item.label}>
                            <span className='mt-1 flex items-center text-xs leading-7 text-secondary'>
                                {item.label}
                            </span>
                            <TextInput
                                className='w-full font-jb text-xs text-tertiary'
                                value={item.example}
                                readonly
                            />
                        </Fragment>
                    )
                })}
            </div>
        </HoverCard>
    )
}
