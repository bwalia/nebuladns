<template>

    <div class="form-input-wrapper">
        <div class="form-input form-input--select">
            
            <select 
                :name="name" 
                :value="value">

                <option value=""></option>

                <option value="option">
                    Option
                </option>
                
            </select>
        
            <label class="form-input-label" v-bind:for="inputId">{{ label }}</label>
            <span class="form-input-bar"></span>
        </div>
        <b class="error-message" v-if="errors">{{ errors }}</b>
    </div>

</template>

<script>
export default {
    props: ["inputid", "label", "name", "value", "errors", "options"],
}
</script>

<style lang="css" scoped>
@import "~/assets/css/variables.css";

.form-input {
    position: relative;
    width: auto;
    display: block;
    min-height: 56px;
    background: #f8f8f8;
    box-shadow: inset 0 0 0 1px #d5d5d5;
    box-sizing: border-box;
}

.form-input select {
    color: var(--default);
    background: transparent;
    position: absolute;
    top: 0;
    left: 0;
    font-size: 16px;
    font-family: var(--font);
    width: 100%;
    box-sizing: border-box;
    line-height: 46px;
    padding: 10px 30px 0 0.8em;
    cursor: text;
    border-radius: 0;
    border: 0;
    outline: none;
    z-index: 3;
    box-shadow: inset 0 0 0 1px #d5d5d5;
    height: 56px;
    appearance:none;
    background-image: url('~assets/images/twisty.svg');
    background-size: 18px;
    background-position: right 15px center;
    background-repeat: no-repeat;
}


.form-input select:focus {
    outline: none;
}

.form-input select:focus ~ .form-input-bar:after {
    transform: scaleX(1);
}


.form-input label {
    text-align: left;
    position: absolute;
    top: 0;
    left: 0;
    color: var(--default);
    font-size: 16px;
    width: 100%;
    line-height: 56px;
    padding: 0 15px;
    z-index: 2;
    transition: 0.2s ease all; 
    transform-origin: 0 0;
    user-select: none;
    max-height: 56px;
    overflow: hidden;
    text-overflow: ellipsis;
}

.form-input-bar:after {
    content: ' ';
    position: absolute;
    bottom: 0;
    left: 0;
    height: 3px;
    background: var(--default);
    width: 100%;
    z-index: 9;
    transition: 0.2s all ease;
    transform-origin: center;
    transform: scaleX(0);
}

.isFilled label {
    transform: translate(2px,-8px) scale(0.75);
    font-weight: bold;
    opacity: 0.3;
}

.error-message {
    display: block;
    color: #c00b23;
    text-align: left;
    font-size: 13px;
}

/* Chrome autofill styles */
input:-webkit-autofill,
input:-webkit-autofill:hover, 
input:-webkit-autofill:focus,
textarea:-webkit-autofill,
textarea:-webkit-autofill:hover,
textarea:-webkit-autofill:focus,
select:-webkit-autofill,
select:-webkit-autofill:hover,
select:-webkit-autofill:focus {
    border: none;
    -webkit-text-fill-color: var(--default);
    -webkit-box-shadow: 0 0 0 1000px rgba(255, 240, 0, 0.18) inset;
    transition: background-color 5000s ease-in-out 0s;
}

</style>
