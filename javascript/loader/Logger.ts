/**
 * Auwgent Logger - Configurable logging for production and development
 * 
 * Usage:
 *   import { logger, LogLevel } from './Logger';
 *   
 *   // Set level (default: INFO in prod, DEBUG in dev)
 *   logger.setLevel(LogLevel.DEBUG);
 *   
 *   // Log messages
 *   logger.debug('[Agent] Processing...', { data });
 *   logger.info('[Agent] Completed');
 *   logger.warn('[Agent] Slow response');
 *   logger.error('[Agent] Failed', error);
 */

export enum LogLevel {
    NONE = 0,
    ERROR = 1,
    WARN = 2,
    INFO = 3,
    DEBUG = 4,
    TRACE = 5
}

export interface TokenUsage {
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
}

export interface AgentStats {
    totalCalls: number;
    helperCalls: Record<string, number>;
    workflowCalls: Record<string, number>;
    toolCalls: Record<string, number>;
    tokenUsage: TokenUsage;
    startTime: number;
    endTime?: number;
}

class Logger {
    private level: LogLevel = LogLevel.INFO;
    private stats: AgentStats = this.createEmptyStats();

    setLevel(level: LogLevel): void {
        this.level = level;
    }

    getLevel(): LogLevel {
        return this.level;
    }

    // Convenience method for setting level by name
    setLevelByName(name: 'none' | 'error' | 'warn' | 'info' | 'debug' | 'trace'): void {
        const levels: Record<string, LogLevel> = {
            none: LogLevel.NONE,
            error: LogLevel.ERROR,
            warn: LogLevel.WARN,
            info: LogLevel.INFO,
            debug: LogLevel.DEBUG,
            trace: LogLevel.TRACE
        };
        this.level = levels[name] ?? LogLevel.INFO;
    }

    private createEmptyStats(): AgentStats {
        return {
            totalCalls: 0,
            helperCalls: {},
            workflowCalls: {},
            toolCalls: {},
            tokenUsage: { promptTokens: 0, completionTokens: 0, totalTokens: 0 },
            startTime: Date.now()
        };
    }

    // ===== Logging methods =====

    error(message: string, ...args: any[]): void {
        if (this.level >= LogLevel.ERROR) {
            console.error(`❌ ${message}`, ...args);
        }
    }

    warn(message: string, ...args: any[]): void {
        if (this.level >= LogLevel.WARN) {
            console.warn(`⚠️ ${message}`, ...args);
        }
    }

    info(message: string, ...args: any[]): void {
        if (this.level >= LogLevel.INFO) {
            console.log(`ℹ️ ${message}`, ...args);
        }
    }

    debug(message: string, ...args: any[]): void {
        if (this.level >= LogLevel.DEBUG) {
            console.log(`🔧 ${message}`, ...args);
        }
    }

    trace(message: string, ...args: any[]): void {
        if (this.level >= LogLevel.TRACE) {
            console.log(`📍 ${message}`, ...args);
        }
    }

    // ===== Stats tracking =====

    resetStats(): void {
        this.stats = this.createEmptyStats();
    }

    trackHelperCall(name: string): void {
        this.stats.totalCalls++;
        this.stats.helperCalls[name] = (this.stats.helperCalls[name] || 0) + 1;
    }

    trackWorkflowCall(name: string): void {
        this.stats.totalCalls++;
        this.stats.workflowCalls[name] = (this.stats.workflowCalls[name] || 0) + 1;
    }

    trackToolCall(name: string): void {
        this.stats.totalCalls++;
        this.stats.toolCalls[name] = (this.stats.toolCalls[name] || 0) + 1;
    }

    trackTokens(usage: Partial<TokenUsage>): void {
        if (usage.promptTokens) {
            this.stats.tokenUsage.promptTokens += usage.promptTokens;
        }
        if (usage.completionTokens) {
            this.stats.tokenUsage.completionTokens += usage.completionTokens;
        }
        if (usage.totalTokens) {
            this.stats.tokenUsage.totalTokens += usage.totalTokens;
        }
    }

    finalize(): void {
        this.stats.endTime = Date.now();
    }

    getStats(): AgentStats {
        return { ...this.stats };
    }

    printStats(): void {
        const stats = this.getStats();
        const duration = (stats.endTime || Date.now()) - stats.startTime;

        console.log('\n📊 Agent Statistics:');
        console.log('─'.repeat(40));
        console.log(`⏱️  Duration: ${duration}ms`);
        console.log(`📞 Total Calls: ${stats.totalCalls}`);

        if (Object.keys(stats.helperCalls).length > 0) {
            console.log(`🤖 Helpers: ${JSON.stringify(stats.helperCalls)}`);
        }
        if (Object.keys(stats.workflowCalls).length > 0) {
            console.log(`🔄 Workflows: ${JSON.stringify(stats.workflowCalls)}`);
        }
        if (Object.keys(stats.toolCalls).length > 0) {
            console.log(`🔧 Tools: ${JSON.stringify(stats.toolCalls)}`);
        }

        console.log(`📝 Tokens: ${stats.tokenUsage.totalTokens} total (${stats.tokenUsage.promptTokens} prompt + ${stats.tokenUsage.completionTokens} completion)`);
        console.log('─'.repeat(40));
    }
}

// Singleton instance
export const logger = new Logger();

// Set default level based on environment
if (typeof process !== 'undefined' && process.env?.NODE_ENV === 'production') {
    logger.setLevel(LogLevel.WARN);
}
